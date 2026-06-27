import type {
	ResourceSummary,
	ResourceTopology,
	TopologyEdge,
	TopologyHealth,
	TopologyNode,
	TopologyRelation,
} from "@/lib/types";
import {
	buildOwnershipFlowTopologyLayout,
	resourceTopologyNodeId,
} from "../topology";

export const TOPOLOGY_SPIKE_NODE_COUNTS = [1_000, 4_000, 10_000] as const;
export const TOPOLOGY_SPIKE_SELECTION_COUNT = 100;

export interface TopologySpikeNode {
	id: string;
	x: number;
	y: number;
	kind: string;
	name: string;
	namespace: string | null;
	status: string;
	health: TopologyHealth;
	label: string;
}

export interface TopologySpikeEdge {
	id: string;
	source: string;
	target: string;
	relation: TopologyRelation;
}

export interface TopologySpikeGraph {
	topology: ResourceTopology;
	nodes: TopologySpikeNode[];
	edges: TopologySpikeEdge[];
	selectionIds: string[];
	layoutMs: number;
}

interface NodeTemplate {
	kind: string;
	apiVersion: string;
	relationToPrevious?: TopologyRelation;
}

const NODE_TEMPLATES: NodeTemplate[] = [
	{ kind: "Ingress", apiVersion: "networking.k8s.io/v1" },
	{ kind: "Service", apiVersion: "v1", relationToPrevious: "routesTo" },
	{ kind: "Deployment", apiVersion: "apps/v1", relationToPrevious: "targets" },
	{ kind: "ReplicaSet", apiVersion: "apps/v1", relationToPrevious: "creates" },
	{ kind: "Pod", apiVersion: "v1", relationToPrevious: "owns" },
];

const HEALTH_SEQUENCE: TopologyHealth[] = [
	"healthy",
	"healthy",
	"attention",
	"restarted",
	"degraded",
];

function namespaceForApp(appIndex: number): string {
	return `namespace-${appIndex % 120}`;
}

function nameFor(template: NodeTemplate, appIndex: number): string {
	const prefix = template.kind.toLowerCase();
	return `${prefix}-api-${appIndex}`;
}

function summaryFor(
	cluster: string,
	template: NodeTemplate,
	appIndex: number,
	namespace: string,
	health: TopologyHealth,
): ResourceSummary {
	return {
		cluster,
		apiVersion: template.apiVersion,
		kind: template.kind,
		name: nameFor(template, appIndex),
		namespace,
		age: `${(appIndex % 59) + 1}m`,
		health,
		status: health === "healthy" ? "Running" : health,
		ready: health === "healthy" ? "True" : "False",
		ownerRef:
			template.kind === "Pod" || template.kind === "ReplicaSet"
				? `deployment-api-${appIndex}`
				: undefined,
		argoApp: `app-${appIndex % 80}`,
		helmRelease: appIndex % 7 === 0 ? `release-${appIndex % 30}` : undefined,
		metrics: {
			kind: template.kind,
			cluster,
			name: nameFor(template, appIndex),
			namespace,
			cpuMillicores: 25 + (appIndex % 900),
			memoryBytes: (64 + (appIndex % 512)) * 1024 * 1024,
			sourcePods: template.kind === "Pod" ? [nameFor(template, appIndex)] : [],
		},
	};
}

function topologyNodeFor(
	cluster: string,
	template: NodeTemplate,
	appIndex: number,
	templateIndex: number,
): TopologyNode {
	const namespace = namespaceForApp(appIndex);
	const health = HEALTH_SEQUENCE[(appIndex + templateIndex) % HEALTH_SEQUENCE.length];
	const summary = summaryFor(cluster, template, appIndex, namespace, health);

	return {
		id: resourceTopologyNodeId(
			cluster,
			template.apiVersion,
			template.kind,
			namespace,
			summary.name,
		),
		kind: template.kind,
		name: summary.name,
		namespace,
		status: summary.status,
		health,
		portHints: template.kind === "Service" ? ["80:8080", "443:8443"] : undefined,
		metrics: summary.metrics,
		selectable: true,
		summary,
	};
}

export function buildSyntheticTopology(nodeCount: number): ResourceTopology {
	const cluster = "spike";
	const nodes: TopologyNode[] = [];
	const edges: TopologyEdge[] = [];
	const appCount = Math.ceil(nodeCount / NODE_TEMPLATES.length);

	for (let appIndex = 0; appIndex < appCount; appIndex += 1) {
		const appNodes = NODE_TEMPLATES.map((template, templateIndex) =>
			topologyNodeFor(cluster, template, appIndex, templateIndex),
		).slice(0, Math.max(0, nodeCount - nodes.length));

		for (const node of appNodes) {
			nodes.push(node);
		}

		for (let templateIndex = 1; templateIndex < appNodes.length; templateIndex += 1) {
			const source = appNodes[templateIndex - 1];
			const target = appNodes[templateIndex];
			const relation =
				NODE_TEMPLATES[templateIndex].relationToPrevious ?? "owns";
			edges.push({
				id: `${source.id}->${target.id}`,
				source: source.id,
				target: target.id,
				relation,
			});
		}
	}

	return { nodes, edges, warnings: [] };
}

function selectionIdsFor(nodes: TopologySpikeNode[]): string[] {
	const podIds = nodes
		.filter((node) => node.kind === "Pod")
		.map((node) => node.id);
	const candidates = podIds.length > 0 ? podIds : nodes.map((node) => node.id);
	const step = Math.max(1, Math.floor(candidates.length / TOPOLOGY_SPIKE_SELECTION_COUNT));
	return candidates
		.filter((_, index) => index % step === 0)
		.slice(0, TOPOLOGY_SPIKE_SELECTION_COUNT);
}

export function createTopologySpikeGraph(nodeCount: number): TopologySpikeGraph {
	const topology = buildSyntheticTopology(nodeCount);
	const started = performance.now();
	const layout = buildOwnershipFlowTopologyLayout(topology, null, {
		groupStandalone: false,
		showPortHints: true,
	});
	const layoutMs = performance.now() - started;
	const nodes = layout.nodes
		.filter((node) => node.type === "ownershipResource")
		.map<TopologySpikeNode>((node) => {
			const topologyNode = node.data.node;
			return {
				id: node.id,
				x: node.position.x,
				y: node.position.y,
				kind: topologyNode.kind,
				name: topologyNode.name,
				namespace: topologyNode.namespace,
				status: topologyNode.status ?? "unknown",
				health: topologyNode.health,
				label: `${topologyNode.kind}/${topologyNode.name}`,
			};
		});

	return {
		topology,
		nodes,
		edges: layout.edges.map((edge) => ({
			id: edge.id,
			source: edge.source,
			target: edge.target,
			relation: edge.data?.relation ?? "owns",
		})),
		selectionIds: selectionIdsFor(nodes),
		layoutMs,
	};
}
