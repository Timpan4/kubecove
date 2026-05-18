import { describe, expect, test } from "bun:test";
import {
	createMockTauriClient,
	getResourceYaml,
	isAppError,
	listResourceTopology,
	listKubeContexts,
	listNamespaces,
} from "../src/lib/tauri";
import type { ClusterContext, NamespaceSummary, ResourceTopology } from "../src/lib/types";

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
});

describe("isAppError", () => {
	test("accepts serialized app errors and rejects partial objects", () => {
		expect(isAppError({ message: "test", kind: "cluster" })).toBe(true);
		expect(isAppError({ message: "test" })).toBe(false);
		expect(isAppError({ kind: "cluster" })).toBe(false);
		expect(isAppError(null)).toBe(false);
	});
});
