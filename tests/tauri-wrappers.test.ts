import { describe, expect, test } from "bun:test";
import {
	createMockTauriClient,
	getAppUsageMetrics,
	getHelmReleaseDetails,
	listRbacInspection,
	getResourceYaml,
	isAppError,
	listHelmReleases,
	listResourceTopology,
	listKubeContexts,
	listNamespaces,
} from "../src/lib/tauri";
import type {
	AppUsageMetrics,
	ClusterContext,
	HelmReleaseDetails,
	HelmReleaseSummary,
	NamespaceSummary,
	RbacInspectionSummary,
	ResourceTopology,
} from "../src/lib/types";

describe("createMockTauriClient", () => {
	test("returns mock response for known command", async () => {
		const mockContexts: ClusterContext[] = [
			{ name: "minikube", isCurrent: true },
			{ name: "docker-desktop", isCurrent: false },
		];
		const client = createMockTauriClient({
			list_kube_contexts: mockContexts,
		});

		expect(await listKubeContexts(client)).toEqual(mockContexts);
	});

	test("throws for unknown command", async () => {
		const client = createMockTauriClient({});

		await expect(listKubeContexts(client)).rejects.toThrow(
			"No mock response for command: list_kube_contexts",
		);
	});
});

describe("typed Tauri wrappers", () => {
	test("passes namespace requests through the typed client", async () => {
		const mockNamespaces: NamespaceSummary[] = [
			{ name: "default", age: "1d" },
		];
		const client = createMockTauriClient({ list_namespaces: mockNamespaces });

		expect(await listNamespaces(client, "minikube")).toEqual(mockNamespaces);
	});

	test("returns raw YAML strings from the resource YAML wrapper", async () => {
		const mockYaml = "apiVersion: v1\nkind: Pod\nmetadata:\n  name: test-pod";
		const client = createMockTauriClient({ get_resource_yaml: mockYaml });

		expect(
			await getResourceYaml(client, "minikube", "Pod", "test-pod", "default"),
		).toBe(mockYaml);
	});

	test("passes topology scope through the typed client", async () => {
		const topology: ResourceTopology = { nodes: [], edges: [], warnings: [] };
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls.push({ cmd, args });
				return topology as T;
			},
		};

		expect(await listResourceTopology(client, "kind-dev", ["default", "payments"])).toEqual(topology);
		expect(calls).toEqual([
			{
				cmd: "list_resource_topology",
				args: {
					clusterContext: "kind-dev",
					namespaces: ["default", "payments"],
				},
			},
		]);
	});

	test("fetches app usage metrics through the typed client", async () => {
		const metrics: AppUsageMetrics = {
			cpuPercent: 2.4,
			memoryBytes: 184 * 1024 * 1024,
			processCount: 3,
			sampledAt: "2026-05-20T10:00:00Z",
			breakdown: [
				{
					label: "WebView",
					description: "Embedded WebView browser runtime",
					cpuPercent: 1.8,
					memoryBytes: 128 * 1024 * 1024,
					processCount: 2,
					children: [
						{
							label: "WebView process 1",
							description: "Embedded WebView browser runtime",
							cpuPercent: 1.2,
							memoryBytes: 96 * 1024 * 1024,
							processCount: 1,
							children: [],
						},
					],
				},
			],
		};
		const client = createMockTauriClient({ get_app_usage_metrics: metrics });

		expect(await getAppUsageMetrics(client)).toEqual(metrics);
	});

	test("passes Helm release requests through typed wrappers", async () => {
		const release: HelmReleaseSummary = {
			cluster: "kind-dev",
			name: "payments",
			namespace: "payments",
			age: "5m",
			chart: "payments-1.2.3",
			appVersion: "2026.5.21",
			revision: 7,
			status: "deployed",
			storageKind: "Secret",
			storageName: "sh.helm.release.v1.payments.v7",
		};
		const details: HelmReleaseDetails = {
			summary: release,
			yaml: "kind: Secret",
			metadata: { name: release.storageName },
		};
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls.push({ cmd, args });
				if (cmd === "list_helm_releases") return [release] as T;
				return details as T;
			},
		};

		expect(await listHelmReleases(client, "kind-dev")).toEqual([release]);
		expect(await getHelmReleaseDetails(client, release)).toEqual(details);
		expect(calls).toEqual([
			{
				cmd: "list_helm_releases",
				args: { clusterContext: "kind-dev" },
			},
			{
				cmd: "get_helm_release_details",
				args: {
					clusterContext: "kind-dev",
					namespace: "payments",
					storageKind: "Secret",
					storageName: "sh.helm.release.v1.payments.v7",
				},
			},
		]);
	});

	test("passes RBAC inspection scope through typed wrappers", async () => {
		const inspection: RbacInspectionSummary = {
			cluster: "kind-dev",
			warnings: [],
			serviceAccounts: [],
			roles: [],
			clusterRoles: [],
			roleBindings: [],
			clusterRoleBindings: [],
			namespaceAccess: [],
		};
		const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
		const client = {
			invoke: async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
				calls.push({ cmd, args });
				return inspection as T;
			},
		};

		expect(await listRbacInspection(client, "kind-dev", ["payments"])).toEqual(inspection);
		expect(calls).toEqual([
			{
				cmd: "list_rbac_inspection",
				args: { clusterContext: "kind-dev", namespaces: ["payments"] },
			},
		]);
	});
});

describe("isAppError", () => {
	test("accepts serialized app errors and rejects partial objects", () => {
		expect(isAppError({ message: "test", kind: "cluster" })).toBe(true);
		expect(isAppError({ message: "test" })).toBe(false);
		expect(isAppError({ kind: "cluster" })).toBe(false);
		expect(isAppError(null)).toBe(false);
	});
});
