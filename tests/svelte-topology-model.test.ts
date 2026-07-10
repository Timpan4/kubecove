import type { ResourceSummary, ResourceTopology, TopologyNode } from "@/lib/types";
import {
	applyFlowTopologySelectionWithIndex,
	buildFlowTopology,
	buildFlowTopologyLayout,
	buildFlowTopologySelectionIndex,
	filterFlowTopologyToSelectedRoot,
	getTopologyTranslateExtent,
	widthFitFlowTopologyViewport,
} from "@/features/resources/topology";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	not: { toContain(expected: unknown): void };
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toContain(expected: unknown): void;
};

function summary(kind: string, name: string): ResourceSummary {
	return {
		kind,
		cluster: "kind-kubecove",
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

function serviceNode(name: string): TopologyNode {
	return {
		...node("Service", name),
		portHints: ["http:80->8080", "metrics:9090->9090", "admin:9000->9000", "extra:1"],
		status: "healthy",
	};
}

describe("svelte topology model", () => {
	test("maps backend topology into selectable Svelte Flow nodes and active edges", () => {
		const topology: ResourceTopology = {
			nodes: [
				node("Deployment", "api"),
				node("ReplicaSet", "api-abc"),
				node("Pod", "api-abc-123"),
			],
			edges: [
				{
					id: "deployment-to-rs",
					source: "Deployment:api",
					target: "ReplicaSet:api-abc",
					relation: "owns",
				},
				{
					id: "rs-to-pod",
					source: "ReplicaSet:api-abc",
					target: "Pod:api-abc-123",
					relation: "creates",
				},
			],
			warnings: [],
		};

		const graph = buildFlowTopology(topology, "Pod:api-abc-123");

		expect(graph.nodes.map((item) => item.id)).toEqual([
			"Deployment:api",
			"ReplicaSet:api-abc",
			"Pod:api-abc-123",
		]);
		expect(graph.nodes[2]?.data.resource?.name).toBe("api-abc-123");
		expect(graph.nodes[2]?.selected).toBe(true);
		expect(graph.nodes[2]?.data.selected).toBe(true);
		expect(graph.nodes[0]?.data.connected).toBe(true);
		expect(graph.edges.find((edge) => edge.id === "rs-to-pod")?.animated).toBe(true);
		expect(graph.edges.find((edge) => edge.id === "deployment-to-rs")?.animated).toBe(true);
		expect(graph.nodes[0]?.type).toBe("ownershipResource");
		expect(graph.nodes[0]?.style).toContain("width: 408px");
		expect(graph.edges.every((edge) => edge.type === "smoothstep")).toBe(true);
		expect(graph.edges.every((edge) => edge.pathOptions?.borderRadius === 10)).toBe(true);
	});

	test("shows backend port hints in Svelte network flow mode", () => {
		const topology: ResourceTopology = {
			nodes: [serviceNode("api")],
			edges: [],
			warnings: [],
		};

		const ownership = buildFlowTopology(topology, null, "ownership");
		const network = buildFlowTopology(topology, null, "networkFlow");

		expect(ownership.nodes[0]?.id).toBe("standalone-kind:Service");
		expect(network.nodes[0]?.data.showPortHints).toBe(true);
		expect(network.nodes[0]?.data.node?.portHints).toContain("http:80->8080");
		expect(network.nodes[0]?.data.node?.portHints).toContain("metrics:9090->9090");
	});

	test("shows compact metrics on Svelte topology nodes", () => {
		const pod = node("Pod", "api-0");
		pod.metrics = {
			kind: "Pod",
			name: "api-0",
			namespace: "argocd",
			cpuMillicores: 125,
			memoryBytes: 64 * 1024 * 1024,
			sampledAt: "2026-05-22T12:00:00Z",
			sourcePods: [],
		};
		const topology: ResourceTopology = {
			nodes: [pod],
			edges: [],
			warnings: [],
		};

		const graph = buildFlowTopology(topology, pod.id);

		expect(graph.nodes.find((item) => item.id === pod.id)?.data.node?.metrics).toEqual(
			pod.metrics,
		);
	});

	test("applies Svelte topology selection without rebuilding layout", () => {
		const topology: ResourceTopology = {
			nodes: [
				node("Deployment", "api"),
				node("ReplicaSet", "api-abc"),
				node("Pod", "api-abc-123"),
			],
			edges: [
				{
					id: "deployment-to-rs",
					source: "Deployment:api",
					target: "ReplicaSet:api-abc",
					relation: "owns",
				},
				{
					id: "rs-to-pod",
					source: "ReplicaSet:api-abc",
					target: "Pod:api-abc-123",
					relation: "creates",
				},
			],
			warnings: [],
		};
		const layout = buildFlowTopologyLayout(topology, null);
		const selected = applyFlowTopologySelectionWithIndex(
			layout,
			buildFlowTopologySelectionIndex(topology),
			"Pod:api-abc-123",
		);

		expect(selected.nodes.map((item) => item.position)).toEqual(
			layout.nodes.map((item) => item.position),
		);
		expect(selected.nodes.find((item) => item.id === "Pod:api-abc-123")?.selected).toBe(
			true,
		);
		expect(selected.nodes.find((item) => item.id === "Deployment:api")?.data.connected).toBe(
			true,
		);
		expect(selected.edges.find((edge) => edge.id === "rs-to-pod")?.animated).toBe(true);
	});

	test("filters Svelte topology selection to the selected root ownership tree", () => {
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
		const filtered = filterFlowTopologyToSelectedRoot(
			buildFlowTopologyLayout(topology, null),
			buildFlowTopologySelectionIndex(topology),
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

	test("builds Svelte topology pan bounds from viewport size", () => {
		const topology: ResourceTopology = {
			nodes: [node("Pod", "api-0")],
			edges: [],
			warnings: [],
		};
		const graph = buildFlowTopology(topology, null, "networkFlow");

		expect(getTopologyTranslateExtent(graph.nodes, { width: 0, height: 640 })).toEqual([
			[0, 0],
			[0, 0],
		]);
		expect(getTopologyTranslateExtent(graph.nodes, { width: 1000, height: 500 })).toEqual([
			[-8657.333333333334, -4280.666666666667],
			[9257.333333333334, 4570.666666666667],
		]);
	});

	test("fits Svelte topology with tight side margins", () => {
		expect(
			widthFitFlowTopologyViewport(
				{ left: 0, top: 0, right: 1000, bottom: 100, width: 1000, height: 100 },
				{ width: 1000, height: 500 },
			),
		).toEqual({ x: 80, y: 208, zoom: 0.84 });
	});
});
