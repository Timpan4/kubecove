import type { ResourceSummary, ResourceTopology, TopologyNode } from "@/lib/types";
import {
	applyReactFlowTopologySelectionWithIndex,
	buildReactFlowTopologyLayout,
	buildReactFlowTopologySelectionIndex,
	filterReactFlowTopologyToSelectedRoot,
} from "./topology";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

function summary(kind: string, name: string): ResourceSummary {
	return {
		kind,
		cluster: "admin@solid-k8s",
		name,
		namespace: "argocd",
		age: "1m",
		apiVersion: "apps/v1",
		health: "healthy",
	};
}

function node(kind: string, name: string): TopologyNode {
	return {
		id: `${kind}:${name}`,
		kind,
		name,
		namespace: "argocd",
		status: "healthy",
		health: "healthy",
		selectable: true,
		summary: summary(kind, name),
	};
}

describe("topology selection", () => {
	test("animates only selected ownership path edges", () => {
		const topology: ResourceTopology = {
			nodes: [
				node("Deployment", "argocd-redis"),
				node("ReplicaSet", "argocd-redis-abc"),
				node("Pod", "argocd-redis-abc-123"),
				node("Secret", "argocd-secret"),
			],
			edges: [
				{
					id: "deployment-to-replicaset",
					source: "Deployment:argocd-redis",
					target: "ReplicaSet:argocd-redis-abc",
					relation: "owns",
				},
				{
					id: "replicaset-to-pod",
					source: "ReplicaSet:argocd-redis-abc",
					target: "Pod:argocd-redis-abc-123",
					relation: "creates",
				},
				{
					id: "deployment-to-secret",
					source: "Deployment:argocd-redis",
					target: "Secret:argocd-secret",
					relation: "targets",
				},
			],
			warnings: [],
		};
		const layout = buildReactFlowTopologyLayout(topology, null, {
			groupStandalone: false,
		});
		const selected = applyReactFlowTopologySelectionWithIndex(
			layout,
			buildReactFlowTopologySelectionIndex(topology),
			"Pod:argocd-redis-abc-123",
		);

		const activeEdge = selected.edges.find(
			(edge) => edge.id === "replicaset-to-pod",
		);
		const inactiveEdge = selected.edges.find(
			(edge) => edge.id === "deployment-to-secret",
		);

		expect(activeEdge?.animated).toBe(true);
		expect(activeEdge?.className).toBe(
			"ownership-map-edge ownership-map-edge-active",
		);
		expect(activeEdge?.zIndex).toBe(10);
		expect(activeEdge?.style?.opacity).toBe(1);
		expect(activeEdge?.style?.strokeWidth).toBe(2.8);
		expect(activeEdge?.style?.strokeDasharray).toBe("5 5");

		expect(inactiveEdge?.animated).toBe(false);
		expect(inactiveEdge?.className).toBe("ownership-map-edge");
		expect(inactiveEdge?.zIndex).toBe(0);
		expect(inactiveEdge?.style?.opacity).toBe(0.16);
		expect(inactiveEdge?.style?.strokeWidth).toBe(1.8);

		expect(
			selected.nodes.find((node) => node.id === "Secret:argocd-secret")?.data
				.dimmed,
		).toBe(true);
		expect(
			selected.nodes.find((node) => node.id === "Pod:argocd-redis-abc-123")
				?.data.selected,
		).toBe(true);
	});

	test("filters React topology selection to the selected root ownership tree", () => {
		const topology: ResourceTopology = {
			nodes: [
				node("Deployment", "api"),
				node("ReplicaSet", "api-abc"),
				node("Pod", "api-abc-1"),
				node("Pod", "api-abc-2"),
				node("Deployment", "web"),
				node("ReplicaSet", "web-abc"),
				node("Pod", "web-abc-1"),
			],
			edges: [
				{
					id: "api-deployment-to-rs",
					source: "Deployment:api",
					target: "ReplicaSet:api-abc",
					relation: "owns",
				},
				{
					id: "api-rs-to-pod-1",
					source: "ReplicaSet:api-abc",
					target: "Pod:api-abc-1",
					relation: "creates",
				},
				{
					id: "api-rs-to-pod-2",
					source: "ReplicaSet:api-abc",
					target: "Pod:api-abc-2",
					relation: "creates",
				},
				{
					id: "web-deployment-to-rs",
					source: "Deployment:web",
					target: "ReplicaSet:web-abc",
					relation: "owns",
				},
				{
					id: "web-rs-to-pod",
					source: "ReplicaSet:web-abc",
					target: "Pod:web-abc-1",
					relation: "creates",
				},
			],
			warnings: [],
		};
		const filtered = filterReactFlowTopologyToSelectedRoot(
			buildReactFlowTopologyLayout(topology, null),
			buildReactFlowTopologySelectionIndex(topology),
			"Pod:api-abc-1",
		);

		expect(filtered.nodes.map((item) => item.id)).toEqual([
			"Deployment:api",
			"ReplicaSet:api-abc",
			"Pod:api-abc-1",
			"Pod:api-abc-2",
		]);
		expect(filtered.edges.map((edge) => edge.id)).toEqual([
			"api-deployment-to-rs",
			"api-rs-to-pod-1",
			"api-rs-to-pod-2",
		]);
	});
});
