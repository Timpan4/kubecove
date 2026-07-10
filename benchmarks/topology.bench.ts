import { bench, describe } from "vitest";
import {
	applyFlowTopologySelectionWithIndex,
	buildFlowTopology,
	buildFlowTopologyLayout,
	buildFlowTopologySelectionIndex,
	resourceTopologyNodeId,
} from "@/features/resources/topology";
import type {
	ResourceSummary,
	ResourceTopology,
	TopologyNode,
} from "@/lib/types";

function summary(kind: string, name: string, namespace: string): ResourceSummary {
	return {
		cluster: "prod",
		kind,
		name,
		namespace,
		age: "1m",
		status: "Running",
		ready: "True",
		apiVersion: kind === "Pod" ? "v1" : "apps/v1",
	};
}

function topologyNode(kind: string, name: string, namespace: string): TopologyNode {
	return {
		id: resourceTopologyNodeId(
			"prod",
			kind === "Pod" ? "v1" : "apps/v1",
			kind,
			namespace,
			name,
		),
		kind,
		name,
		namespace,
		status: "Running",
		health: "healthy",
		selectable: true,
		summary: summary(kind, name, namespace),
	};
}

function buildTopology(apps: number): ResourceTopology {
	const nodes: TopologyNode[] = [];
	const edges: ResourceTopology["edges"] = [];
	for (let index = 0; index < apps; index += 1) {
		const namespace = `namespace-${index % 100}`;
		const deployment = topologyNode("Deployment", `api-${index}`, namespace);
		const pod = topologyNode("Pod", `api-${index}-pod`, namespace);
		nodes.push(deployment, pod);
		edges.push({
			id: `edge-${index}`,
			source: deployment.id,
			target: pod.id,
			relation: "owns",
		});
	}
	return { nodes, edges, warnings: [] };
}

const topology = buildTopology(500);
const selectedIds = topology.nodes
	.filter((node) => node.kind === "Pod")
	.slice(0, 50)
	.map((node) => node.id);

const layout = buildFlowTopologyLayout(topology, null);
const selectionIndex = buildFlowTopologySelectionIndex(topology);

// The full-rebuild path is the most expensive code path, so a smaller slice of
// selections keeps the instrumented run reasonable while staying representative.
const rebuildSelectedIds = selectedIds.slice(0, 10);

describe("ownership flow topology (500 apps / 1k nodes)", () => {
	bench("buildFlowTopology per selection (rebuild)", () => {
		for (const selectedId of rebuildSelectedIds) {
			buildFlowTopology(topology, selectedId);
		}
	});

	bench("buildFlowTopologyLayout (build once)", () => {
		buildFlowTopologyLayout(topology, null);
	});

	bench("applyFlowTopologySelectionWithIndex (indexed selection)", () => {
		for (const selectedId of selectedIds) {
			applyFlowTopologySelectionWithIndex(layout, selectionIndex, selectedId);
		}
	});
});
