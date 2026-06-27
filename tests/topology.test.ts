import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import type { ResourceSummary, ResourceTopology } from "../src/lib/types";
import {
	buildOwnershipFlowTopology,
	buildTopologyRows,
	resourceTopologyNodeId,
	topologyNodeClassName,
	topologySelectableResource,
} from "../src/features/resources/topology";

function summary(overrides: Partial<ResourceSummary>): ResourceSummary {
	return {
		cluster: "kind-dev",
		kind: "Pod",
		name: "api-0",
		namespace: "default",
		age: "1m",
		status: "Running",
		ready: "true",
		...overrides,
	};
}

describe("ownership topology helpers", () => {
	test("keeps shared topology helpers independent from renderer runtime types", () => {
		for (const path of [
			"src/features/resources/topology.ts",
			"src/features/resources/topology-layout.ts",
			"src/features/resources/topology-standalone-groups.ts",
			"src/features/resources/topology-viewport.ts",
		]) {
			const source = readFileSync(path, "utf8");
			expect(source).not.toContain("@xyflow/svelte");
		}
	});

	test("converts topology into stable left-to-right flow nodes and edges", () => {
		const deploymentId = resourceTopologyNodeId(
			"kind-dev",
			"apps/v1",
			"Deployment",
			"default",
			"api",
		);
		const replicaSetId = resourceTopologyNodeId(
			"kind-dev",
			"apps/v1",
			"ReplicaSet",
			"default",
			"api-7d9",
		);
		const podId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"Pod",
			"default",
			"api-7d9-x",
		);
		const topology: ResourceTopology = {
			nodes: [
				{
					id: podId,
					kind: "Pod",
					name: "api-7d9-x",
					namespace: "default",
					status: "Running",
					health: "healthy",
					selectable: true,
					summary: summary({ kind: "Pod", name: "api-7d9-x" }),
				},
				{
					id: deploymentId,
					kind: "Deployment",
					name: "api",
					namespace: "default",
					status: "Available: 1",
					health: "healthy",
					selectable: true,
					summary: summary({ kind: "Deployment", name: "api", apiVersion: "apps/v1" }),
				},
				{
					id: replicaSetId,
					kind: "ReplicaSet",
					name: "api-7d9",
					namespace: "default",
					status: "Available: 1",
					health: "healthy",
					selectable: false,
					summary: summary({ kind: "ReplicaSet", name: "api-7d9", apiVersion: "apps/v1" }),
				},
			],
			edges: [
				{
					id: "deploy-rs",
					source: deploymentId,
					target: replicaSetId,
					relation: "owns",
				},
				{
					id: "rs-pod",
					source: replicaSetId,
					target: podId,
					relation: "owns",
				},
			],
			warnings: [],
		};

		const graph = buildOwnershipFlowTopology(topology, podId);
		const deployment = graph.nodes.find((node) => node.id === deploymentId);
		const replicaSet = graph.nodes.find((node) => node.id === replicaSetId);
		const pod = graph.nodes.find((node) => node.id === podId);

		expect(graph.nodes.map((node) => node.id)).toEqual([
			deploymentId,
			replicaSetId,
			podId,
		]);
		expect(deployment?.position.x).toBeLessThan(replicaSet?.position.x ?? 0);
		expect(replicaSet?.position.x).toBeLessThan(pod?.position.x ?? 0);
		expect(pod?.selected).toBe(true);
		expect(pod?.data.selected).toBe(true);
		expect(pod?.data.showPortHints).toBe(false);
		expect(deployment?.data.connected).toBe(true);
		expect(replicaSet?.data.connected).toBe(true);
		expect(graph.edges.map((edge) => [edge.source, edge.target])).toEqual([
			[deploymentId, replicaSetId],
			[replicaSetId, podId],
		]);
		expect(graph.edges.every((edge) => edge.type === "smoothstep")).toBe(true);
		expect(graph.edges.every((edge) => edge.pathOptions?.borderRadius === 10)).toBe(
			true,
		);
		expect(graph.edges.find((edge) => edge.id === "rs-pod")?.style?.stroke).toBe(
			"var(--primary)",
		);
		expect(graph.edges.find((edge) => edge.id === "rs-pod")?.zIndex).toBe(10);
		expect(graph.edges.find((edge) => edge.id === "deploy-rs")?.style?.opacity).toBe(
			1,
		);
	});

	test("keeps degraded node health available for custom node styling", () => {
		const podId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"Pod",
			"default",
			"api-0",
		);
		const topology: ResourceTopology = {
			nodes: [
				{
					id: podId,
					kind: "Pod",
					name: "api-0",
					namespace: "default",
					status: "Failed",
					health: "degraded",
					selectable: true,
					summary: summary({ status: "Failed" }),
				},
			],
			edges: [],
			warnings: [],
		};

		const graph = buildOwnershipFlowTopology(topology, podId);
		const podNode = graph.nodes.find((node) => node.id === podId);

		expect(podNode?.data.node.health).toBe("degraded");
		expect(topologyNodeClassName(podNode?.data.node ?? topology.nodes[0], null)).toContain(
			"resource-topology-node-health-degraded",
		);
	});

	test("collapses ownerless standalone resources into type buckets by default", () => {
		const deploymentId = resourceTopologyNodeId(
			"kind-dev",
			"apps/v1",
			"Deployment",
			"default",
			"api",
		);
		const serviceId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"Service",
			"default",
			"api",
		);
		const configMapId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"ConfigMap",
			"default",
			"kube-root-ca.crt",
		);
		const appConfigMapId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"ConfigMap",
			"default",
			"api-config",
		);
		const topology: ResourceTopology = {
			nodes: [
				{
					id: serviceId,
					kind: "Service",
					name: "api",
					namespace: "default",
					status: null,
					health: "unknown",
					selectable: true,
					summary: summary({ kind: "Service", name: "api" }),
				},
				{
					id: configMapId,
					kind: "ConfigMap",
					name: "kube-root-ca.crt",
					namespace: "default",
					status: null,
					health: "unknown",
					selectable: true,
					summary: summary({ kind: "ConfigMap", name: "kube-root-ca.crt" }),
				},
				{
					id: appConfigMapId,
					kind: "ConfigMap",
					name: "api-config",
					namespace: "default",
					status: null,
					health: "unknown",
					selectable: true,
					summary: summary({ kind: "ConfigMap", name: "api-config" }),
				},
				{
					id: deploymentId,
					kind: "Deployment",
					name: "api",
					namespace: "default",
					status: "Available: 1",
					health: "healthy",
					selectable: true,
					summary: summary({ kind: "Deployment", name: "api", apiVersion: "apps/v1" }),
				},
			],
			edges: [],
			warnings: [],
		};

		const graph = buildOwnershipFlowTopology(topology, null);
		const groupIds = graph.nodes
			.filter((node) => node.type === "standaloneKindGroup")
			.map((node) => node.id);
		const resourceNodes = graph.nodes.filter(
			(node) => node.type === "ownershipResource",
		);
		const configMapGroup = graph.nodes.find(
			(node) => node.id === "standalone-kind:ConfigMap",
		);

		expect(groupIds).toEqual([
			"standalone-kind:ConfigMap",
			"standalone-kind:Deployment",
			"standalone-kind:Service",
		]);
		expect(resourceNodes.map((node) => node.id)).toEqual([]);
		expect(configMapGroup?.data.count).toBe(2);
		expect(configMapGroup?.data.expanded).toBe(false);
		expect(configMapGroup?.style?.height).toBe(42);
	});

	test("keeps network flow nodes ungrouped when standalone grouping is disabled", () => {
		const serviceId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"Service",
			"default",
			"api",
		);
		const podId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"Pod",
			"default",
			"api-0",
		);
		const topology: ResourceTopology = {
			nodes: [
				{
					id: serviceId,
					kind: "Service",
					name: "api",
					namespace: "default",
					status: null,
					health: "unknown",
					selectable: true,
					summary: summary({ kind: "Service", name: "api" }),
				},
				{
					id: podId,
					kind: "Pod",
					name: "api-0",
					namespace: "default",
					status: "Running",
					health: "healthy",
					selectable: true,
					summary: summary({ kind: "Pod", name: "api-0" }),
				},
			],
			edges: [
				{
					id: "svc-pod",
					source: serviceId,
					target: podId,
					relation: "selects",
				},
			],
			warnings: [],
		};

		const graph = buildOwnershipFlowTopology(topology, null, {
			groupStandalone: false,
			showPortHints: true,
		});

		expect(graph.nodes.map((node) => node.id)).toEqual([serviceId, podId]);
		expect(graph.nodes.every((node) => node.type === "ownershipResource")).toBe(
			true,
		);
		expect(graph.edges[0]?.style?.strokeDasharray).toBe("5 5");
		expect(graph.nodes[0]?.data.showPortHints).toBe(true);
	});

	test("expands ownerless type buckets when requested", () => {
		const configMapId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"ConfigMap",
			"default",
			"kube-root-ca.crt",
		);
		const appConfigMapId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"ConfigMap",
			"default",
			"api-config",
		);
		const topology: ResourceTopology = {
			nodes: [
				{
					id: configMapId,
					kind: "ConfigMap",
					name: "kube-root-ca.crt",
					namespace: "default",
					status: null,
					health: "unknown",
					selectable: true,
					summary: summary({ kind: "ConfigMap", name: "kube-root-ca.crt" }),
				},
				{
					id: appConfigMapId,
					kind: "ConfigMap",
					name: "api-config",
					namespace: "default",
					status: null,
					health: "unknown",
					selectable: true,
					summary: summary({ kind: "ConfigMap", name: "api-config" }),
				},
			],
			edges: [],
			warnings: [],
		};

		const graph = buildOwnershipFlowTopology(topology, null, {
			expandedStandaloneKinds: new Set(["ConfigMap"]),
		});
		const configMapGroup = graph.nodes.find(
			(node) => node.id === "standalone-kind:ConfigMap",
		);
		const resourceNodes = graph.nodes.filter(
			(node) => node.type === "ownershipResource",
		);
		const configMap = graph.nodes.find((node) => node.id === configMapId);
		const appConfigMap = graph.nodes.find((node) => node.id === appConfigMapId);

		expect(configMapGroup?.data.expanded).toBe(true);
		expect(resourceNodes.map((node) => node.id)).toEqual([
			appConfigMapId,
			configMapId,
		]);
		expect(configMap?.parentId).toBe("standalone-kind:ConfigMap");
		expect(appConfigMap?.parentId).toBe("standalone-kind:ConfigMap");
		expect(configMap?.data.standalone).toBe(true);
		expect(configMap?.style?.width).toBe(260);
		expect(configMap?.position.y).toBe(appConfigMap?.position.y);
		expect(configMap?.position.x).toBeGreaterThan(appConfigMap?.position.x ?? 0);
	});

	test("expands a collapsed ownerless bucket for the selected resource", () => {
		const secretId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"Secret",
			"default",
			"bootstrap-token",
		);
		const topology: ResourceTopology = {
			nodes: [
				{
					id: secretId,
					kind: "Secret",
					name: "bootstrap-token",
					namespace: "default",
					status: null,
					health: "unknown",
					selectable: true,
					summary: summary({ kind: "Secret", name: "bootstrap-token" }),
				},
			],
			edges: [],
			warnings: [],
		};

		const graph = buildOwnershipFlowTopology(topology, secretId);
		const secretGroup = graph.nodes.find(
			(node) => node.id === "standalone-kind:Secret",
		);
		const secret = graph.nodes.find((node) => node.id === secretId);

		expect(secretGroup?.data.expanded).toBe(true);
		expect(secret?.parentId).toBe("standalone-kind:Secret");
		expect(secret?.selected).toBe(true);
	});

	test("deduplicates repeated topology nodes before flow render", () => {
		const serviceId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"Service",
			"default",
			"api",
		);
		const serviceNode = {
			id: serviceId,
			kind: "Service",
			name: "api",
			namespace: "default",
			status: null,
			health: "unknown",
			selectable: true,
			summary: summary({ kind: "Service", name: "api" }),
		};
		const topology: ResourceTopology = {
			nodes: [serviceNode, serviceNode],
			edges: [],
			warnings: [],
		};

		const graph = buildOwnershipFlowTopology(topology, null);

		expect(graph.nodes.map((node) => node.id)).toEqual(["standalone-kind:Service"]);
		expect(graph.nodes.find((node) => node.id === "standalone-kind:Service")?.data.count).toBe(
			1,
		);
	});

	test("groups child nodes by owner order to reduce crossing edges", () => {
		const alloyId = resourceTopologyNodeId(
			"kind-dev",
			"apps/v1",
			"DaemonSet",
			"default",
			"alloy",
		);
		const alloyInClusterId = resourceTopologyNodeId(
			"kind-dev",
			"apps/v1",
			"DaemonSet",
			"default",
			"alloy-in-cluster",
		);
		const alloyPodIds = ["alloy-bqpd5", "alloy-vpfgt", "alloy-vt5h"].map(
			(name) => resourceTopologyNodeId("kind-dev", "v1", "Pod", "default", name),
		);
		const alloyInClusterPodIds = [
			"alloy-in-cluster-2rdvp",
			"alloy-in-cluster-dkw5f",
			"alloy-in-cluster-pzlbf",
		].map((name) => resourceTopologyNodeId("kind-dev", "v1", "Pod", "default", name));
		const podNode = (id: string, name: string) => ({
			id,
			kind: "Pod",
			name,
			namespace: "default",
			status: "Running",
			health: "healthy",
			selectable: true,
			summary: summary({ kind: "Pod", name }),
		});
		const topology: ResourceTopology = {
			nodes: [
				{
					id: alloyId,
					kind: "DaemonSet",
					name: "alloy",
					namespace: "default",
					status: "Available: 3",
					health: "healthy",
					selectable: true,
					summary: summary({ kind: "DaemonSet", name: "alloy", apiVersion: "apps/v1" }),
				},
				{
					id: alloyInClusterId,
					kind: "DaemonSet",
					name: "alloy-in-cluster",
					namespace: "default",
					status: "Available: 3",
					health: "healthy",
					selectable: true,
					summary: summary({
						kind: "DaemonSet",
						name: "alloy-in-cluster",
						apiVersion: "apps/v1",
					}),
				},
				...alloyPodIds.map((id) => podNode(id, id.split(":").at(-1) ?? "")),
				...alloyInClusterPodIds.map((id) =>
					podNode(id, id.split(":").at(-1) ?? ""),
				),
			],
			edges: [
				...alloyPodIds.map((target, index) => ({
					id: `alloy-${index}`,
					source: alloyId,
					target,
					relation: "owns" as const,
				})),
				...alloyInClusterPodIds.map((target, index) => ({
					id: `alloy-in-cluster-${index}`,
					source: alloyInClusterId,
					target,
					relation: "owns" as const,
				})),
			],
			warnings: [],
		};

		const graph = buildOwnershipFlowTopology(topology, null);
		const yById = new Map(graph.nodes.map((node) => [node.id, node.position.y]));
		const sortedAlloyYs = alloyPodIds
			.map((id) => yById.get(id) ?? 0)
			.sort((a, b) => a - b);
		const maxAlloyY = Math.max(...alloyPodIds.map((id) => yById.get(id) ?? 0));
		const minAlloyInClusterY = Math.min(
			...alloyInClusterPodIds.map((id) => yById.get(id) ?? 0),
		);

		expect(maxAlloyY).toBeLessThan(minAlloyInClusterY);
		expect(sortedAlloyYs[1] - sortedAlloyYs[0]).toBeGreaterThan(70);
		expect(sortedAlloyYs[1] - sortedAlloyYs[0]).toBeLessThan(96);
	});

	test("adds Argo-style vertical gaps around nested ownership bands", () => {
		const ciliumId = resourceTopologyNodeId(
			"kind-dev",
			"apps/v1",
			"DaemonSet",
			"kube-system",
			"cilium",
		);
		const hubbleId = resourceTopologyNodeId(
			"kind-dev",
			"apps/v1",
			"Deployment",
			"kube-system",
			"hubble-relay",
		);
		const hubbleReplicaSetId = resourceTopologyNodeId(
			"kind-dev",
			"apps/v1",
			"ReplicaSet",
			"kube-system",
			"hubble-relay-7b7",
		);
		const hubblePodId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"Pod",
			"kube-system",
			"hubble-relay-7b7-x",
		);
		const ciliumPodId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"Pod",
			"kube-system",
			"cilium-dmpnh",
		);
		const metricsId = resourceTopologyNodeId(
			"kind-dev",
			"apps/v1",
			"Deployment",
			"kube-system",
			"metrics-server",
		);
		const metricsPodId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"Pod",
			"kube-system",
			"metrics-server-x",
		);
		const node = (
			id: string,
			kind: ResourceSummary["kind"],
			name: string,
			selectable = true,
		) => ({
			id,
			kind,
			name,
			namespace: "kube-system",
			status: "Running",
			health: "healthy" as const,
			selectable,
			summary: summary({ kind, name, namespace: "kube-system" }),
		});
		const topology: ResourceTopology = {
			nodes: [
				node(ciliumId, "DaemonSet", "cilium"),
				node(hubbleId, "Deployment", "hubble-relay"),
				node(hubbleReplicaSetId, "ReplicaSet", "hubble-relay-7b7", false),
				node(hubblePodId, "Pod", "hubble-relay-7b7-x"),
				node(ciliumPodId, "Pod", "cilium-dmpnh"),
				node(metricsId, "Deployment", "metrics-server"),
				node(metricsPodId, "Pod", "metrics-server-x"),
			],
			edges: [
				{ id: "cilium-hubble", source: ciliumId, target: hubbleId, relation: "groups" },
				{
					id: "hubble-rs",
					source: hubbleId,
					target: hubbleReplicaSetId,
					relation: "owns",
				},
				{
					id: "hubble-pod",
					source: hubbleReplicaSetId,
					target: hubblePodId,
					relation: "owns",
				},
				{ id: "cilium-pod", source: ciliumId, target: ciliumPodId, relation: "owns" },
				{
					id: "metrics-pod",
					source: metricsId,
					target: metricsPodId,
					relation: "owns",
				},
			],
			warnings: [],
		};

		const graph = buildOwnershipFlowTopology(topology, null);
		const yById = new Map(graph.nodes.map((graphNode) => [graphNode.id, graphNode.position.y]));
		const hubblePodY = yById.get(hubblePodId) ?? 0;
		const ciliumPodY = yById.get(ciliumPodId) ?? 0;
		const metricsY = yById.get(metricsId) ?? 0;
		const nestedBandGap = ciliumPodY - hubblePodY;
		const rootGroupGap = metricsY - ciliumPodY;

		expect(nestedBandGap).toBeGreaterThanOrEqual(120);
		expect(nestedBandGap).toBeLessThan(150);
		expect(rootGroupGap).toBeGreaterThanOrEqual(90);
		expect(rootGroupGap).toBeLessThan(130);
		expect(yById.get(ciliumId)).toBeGreaterThan(hubblePodY);
		expect(yById.get(ciliumId)).toBeLessThan(ciliumPodY);
	});

	test("selecting a parent highlights the full descendant path", () => {
		const deploymentId = resourceTopologyNodeId(
			"kind-dev",
			"apps/v1",
			"Deployment",
			"default",
			"api",
		);
		const replicaSetId = resourceTopologyNodeId(
			"kind-dev",
			"apps/v1",
			"ReplicaSet",
			"default",
			"api-7d9",
		);
		const podId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"Pod",
			"default",
			"api-7d9-x",
		);
		const standaloneId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"ConfigMap",
			"default",
			"kube-root-ca.crt",
		);
		const topology: ResourceTopology = {
			nodes: [
				{
					id: deploymentId,
					kind: "Deployment",
					name: "api",
					namespace: "default",
					status: "Available: 1",
					health: "healthy",
					selectable: true,
					summary: summary({ kind: "Deployment", name: "api", apiVersion: "apps/v1" }),
				},
				{
					id: replicaSetId,
					kind: "ReplicaSet",
					name: "api-7d9",
					namespace: "default",
					status: "Available: 1",
					health: "healthy",
					selectable: false,
					summary: summary({ kind: "ReplicaSet", name: "api-7d9", apiVersion: "apps/v1" }),
				},
				{
					id: podId,
					kind: "Pod",
					name: "api-7d9-x",
					namespace: "default",
					status: "Running",
					health: "healthy",
					selectable: true,
					summary: summary({ kind: "Pod", name: "api-7d9-x" }),
				},
				{
					id: standaloneId,
					kind: "ConfigMap",
					name: "kube-root-ca.crt",
					namespace: "default",
					status: null,
					health: "unknown",
					selectable: true,
					summary: summary({ kind: "ConfigMap", name: "kube-root-ca.crt" }),
				},
			],
			edges: [
				{ id: "deploy-rs", source: deploymentId, target: replicaSetId, relation: "owns" },
				{ id: "rs-pod", source: replicaSetId, target: podId, relation: "owns" },
			],
			warnings: [],
		};

		const graph = buildOwnershipFlowTopology(topology, deploymentId);
		const nodesById = new Map(graph.nodes.map((node) => [node.id, node]));

		expect(nodesById.get(deploymentId)?.data.selected).toBe(true);
		expect(nodesById.get(replicaSetId)?.data.connected).toBe(true);
		expect(nodesById.get(podId)?.data.connected).toBe(true);
		expect(nodesById.get("standalone-kind:ConfigMap")?.data.dimmed).toBe(true);
		expect(graph.edges.every((edge) => edge.style?.opacity === 1)).toBe(true);
	});

	test("dims standalone buckets when only owned nodes of the same kind are selected", () => {
		const deploymentId = resourceTopologyNodeId(
			"kind-dev",
			"apps/v1",
			"Deployment",
			"default",
			"api",
		);
		const ownedPodId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"Pod",
			"default",
			"api-7d9-x",
		);
		const standalonePodId = resourceTopologyNodeId(
			"kind-dev",
			"v1",
			"Pod",
			"default",
			"debug-shell",
		);
		const topology: ResourceTopology = {
			nodes: [
				{
					id: deploymentId,
					kind: "Deployment",
					name: "api",
					namespace: "default",
					status: "Available",
					health: "healthy",
					selectable: true,
					summary: summary({ kind: "Deployment", name: "api", apiVersion: "apps/v1" }),
				},
				{
					id: ownedPodId,
					kind: "Pod",
					name: "api-7d9-x",
					namespace: "default",
					status: "Running",
					health: "healthy",
					selectable: true,
					summary: summary({ kind: "Pod", name: "api-7d9-x" }),
				},
				{
					id: standalonePodId,
					kind: "Pod",
					name: "debug-shell",
					namespace: "default",
					status: "Running",
					health: "healthy",
					selectable: true,
					summary: summary({ kind: "Pod", name: "debug-shell" }),
				},
			],
			edges: [
				{ id: "deploy-pod", source: deploymentId, target: ownedPodId, relation: "owns" },
			],
			warnings: [],
		};

		const graph = buildOwnershipFlowTopology(topology, deploymentId);
		const bucket = graph.nodes.find((node) => node.id === "standalone-kind:Pod");

		expect(bucket?.data.dimmed).toBe(true);
	});

	test("builds deterministic rows with parent resources before children", () => {
		const topology: ResourceTopology = {
			nodes: [
				{
					id: resourceTopologyNodeId("kind-dev", "v1", "Pod", "default", "api-0"),
					kind: "Pod",
					name: "api-0",
					namespace: "default",
					status: "Running",
					health: "healthy",
					selectable: true,
					summary: summary({ kind: "Pod", name: "api-0" }),
				},
				{
					id: resourceTopologyNodeId("kind-dev", "apps/v1", "Deployment", "default", "api"),
					kind: "Deployment",
					name: "api",
					namespace: "default",
					status: "Available",
					health: "healthy",
					selectable: true,
					summary: summary({ kind: "Deployment", name: "api", apiVersion: "apps/v1" }),
				},
			],
			edges: [
				{
					id: "edge-1",
					source: resourceTopologyNodeId("kind-dev", "apps/v1", "Deployment", "default", "api"),
					target: resourceTopologyNodeId("kind-dev", "v1", "Pod", "default", "api-0"),
					relation: "owns",
				},
			],
			warnings: [],
		};

		const rows = buildTopologyRows(topology);

		expect(rows.map((row) => row.node.kind)).toEqual(["Deployment", "Pod"]);
		expect(rows[1].depth).toBe(1);
		expect(rows[1].parentIds).toEqual([rows[0].node.id]);
	});

	test("maps topology node health and selection into stable class names", () => {
		const node = {
			id: "node-1",
			kind: "Pod",
			name: "api-0",
			namespace: "default",
			status: "Failed",
			health: "degraded" as const,
			selectable: true,
			summary: summary({ status: "Failed" }),
		};

		expect(topologyNodeClassName(node, null)).toContain(
			"resource-topology-node-health-degraded",
		);
		expect(topologyNodeClassName(node, "node-1")).toContain(
			"resource-topology-node-selected",
		);
		expect(topologyNodeClassName(node, "node-1")).toContain("ring-primary");
		expect(topologyNodeClassName(node, null, "node-1", true)).toContain(
			"resource-topology-node-connected",
		);
		expect(
			topologyNodeClassName({ ...node, health: "healthy" }, "node-2"),
		).toContain("border-border");
	});

	test("returns only selectable node resources for detail panel sync", () => {
		const selectable = {
			id: "pod",
			kind: "Pod",
			name: "api-0",
			namespace: "default",
			status: "Running",
			health: "healthy" as const,
			selectable: true,
			summary: summary({ kind: "Pod" }),
		};
		const intermediate = {
			...selectable,
			id: "rs",
			kind: "ReplicaSet",
			selectable: false,
			summary: summary({ kind: "ReplicaSet" }),
		};

		expect(topologySelectableResource(selectable)).toEqual(selectable.summary);
		expect(topologySelectableResource(intermediate)).toBeNull();
	});
});
