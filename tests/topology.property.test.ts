import { describe, expect, test } from "bun:test";
import fc from "fast-check";
import type {
	ResourceSummary,
	ResourceTopology,
	TopologyNode,
} from "../src/lib/types";
import {
	buildFlowTopologyLayout,
	resourceTopologyNodeId,
} from "../src/features/resources/topology-implementation";

const propertyConfig = { seed: 20260521, numRuns: 50 };

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

function node(
	kind: string,
	name: string,
	apiVersion: string,
	selectable: boolean,
): TopologyNode {
	const resource = summary({ kind, name, apiVersion });
	return {
		id: resourceTopologyNodeId(
			resource.cluster,
			apiVersion,
			kind,
			resource.namespace,
			name,
		),
		kind,
		name,
		namespace: resource.namespace,
		status: resource.status,
		health: "healthy",
		selectable,
		summary: resource,
	};
}

function buildGeneratedTopology(podCounts: number[]): ResourceTopology {
	const deployment = node("Deployment", "api", "apps/v1", true);
	const nodes: TopologyNode[] = [deployment];
	const edges: ResourceTopology["edges"] = [];

	for (const [replicaSetIndex, podCount] of podCounts.entries()) {
		const replicaSet = node(
			"ReplicaSet",
			`api-${replicaSetIndex}`,
			"apps/v1",
			false,
		);
		nodes.push(replicaSet);
		edges.push({
			id: `${deployment.id}->${replicaSet.id}`,
			source: deployment.id,
			target: replicaSet.id,
			relation: "owns",
		});

		for (let podIndex = 0; podIndex < podCount; podIndex += 1) {
			const pod = node("Pod", `api-${replicaSetIndex}-${podIndex}`, "v1", true);
			nodes.push(pod);
			edges.push({
				id: `${replicaSet.id}->${pod.id}`,
				source: replicaSet.id,
				target: pod.id,
				relation: "owns",
			});
		}
	}

	return { nodes, edges, warnings: [] };
}

describe("topology properties", () => {
	test("generated topologies keep stable nodes and valid edges", () => {
		fc.assert(
			fc.property(
				fc.array(fc.integer({ min: 0, max: 3 }), { minLength: 0, maxLength: 4 }),
				(podCounts) => {
					const topology = buildGeneratedTopology(podCounts);
					const graph = buildFlowTopologyLayout(topology, null);
					const secondGraph = buildFlowTopologyLayout(topology, null);
					const graphNodeIds = graph.nodes.map((graphNode) => graphNode.id);
					const graphNodeIdSet = new Set(graphNodeIds);

					expect(graphNodeIds).toEqual(secondGraph.nodes.map((graphNode) => graphNode.id));
					expect(graphNodeIdSet.size).toBe(graph.nodes.length);

					for (const edge of graph.edges) {
						expect(graphNodeIdSet.has(edge.source)).toBe(true);
						expect(graphNodeIdSet.has(edge.target)).toBe(true);
					}
				},
			),
			propertyConfig,
		);
	});
});
