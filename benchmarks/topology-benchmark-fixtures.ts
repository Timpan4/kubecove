import { resourceTopologyNodeId } from "@/features/resources/topology-implementation";
import type {
	ResourceSummary,
	ResourceTopology,
	TopologyNode,
} from "@/lib/types";

function summary(
	kind: string,
	name: string,
	namespace: string | null,
): ResourceSummary {
	return {
		cluster: "prod",
		kind,
		name,
		namespace,
		age: "1m",
		createdAt: "2026-06-01T12:00:00Z",
		status: "Running",
		health: "healthy",
		ready: "True",
		apiVersion:
			kind === "Pod" || kind === "Service" || kind === "Node" || kind === "ConfigMap"
				? "v1"
				: "apps/v1",
	};
}

function node(
	kind: string,
	name: string,
	namespace: string | null,
	resourceOverrides: Partial<ResourceSummary> = {},
): TopologyNode {
	const resource = { ...summary(kind, name, namespace), ...resourceOverrides };
	return {
		id: resourceTopologyNodeId(
			"prod",
			resource.apiVersion ?? "v1",
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
		summary: resource,
	};
}

export function buildOwnershipTopology(apps: number): ResourceTopology {
	const nodes: TopologyNode[] = [];
	const edges: ResourceTopology["edges"] = [];
	for (let index = 0; index < apps; index += 1) {
		const namespace = `namespace-${index % 100}`;
		const deployment = node("Deployment", `api-${index}`, namespace);
		const pod = node("Pod", `api-${index}-pod`, namespace);
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

export function addStandaloneTopologyResources(
	topology: ResourceTopology,
): ResourceTopology {
	return {
		...topology,
		nodes: [
			...topology.nodes,
			node("Node", "cluster-standalone", null),
			node("Widget", "custom-standalone", "operator-system", {
				apiVersion: "operator.example.com/v1beta1",
				group: "operator.example.com",
				version: "v1beta1",
				plural: "widgets",
				namespaced: true,
				dynamic: true,
			}),
		],
	};
}

export function buildBranchingTopology(apps: number): ResourceTopology {
	const nodes: TopologyNode[] = [];
	const edges: ResourceTopology["edges"] = [];
	for (let index = 0; index < apps; index += 1) {
		const namespace = `namespace-${index % 100}`;
		const cronJob = node("CronJob", `batch-${index}`, namespace);
		nodes.push(cronJob);
		for (let replica = 0; replica < 2; replica += 1) {
			const job = node("Job", `batch-${index}-${replica}`, namespace);
			nodes.push(job);
			edges.push({
				id: `owns-job-${index}-${replica}`,
				source: cronJob.id,
				target: job.id,
				relation: "owns",
			});
			for (let podIndex = 0; podIndex < 2; podIndex += 1) {
				const pod = node(
					"Pod",
					`batch-${index}-${replica}-${podIndex}`,
					namespace,
				);
				nodes.push(pod);
				edges.push({
					id: `owns-pod-${index}-${replica}-${podIndex}`,
					source: job.id,
					target: pod.id,
					relation: "owns",
				});
			}
		}
	}

	for (let index = 0; index < apps; index += 1) {
		const namespace = `namespace-${index % 100}`;
		const service = node("Service", `batch-${index}`, namespace);
		const pod = node("Pod", `batch-${index}-0-0`, namespace);
		nodes.push(service);
		edges.push({
			id: `selects-${index}`,
			source: service.id,
			target: pod.id,
			relation: "selects",
		});
	}
	for (let index = 0; index < 100; index += 1) {
		nodes.push(
			node("ConfigMap", `standalone-${index}`, `namespace-${index % 100}`),
		);
	}

	return { nodes, edges, warnings: [] };
}

export function buildReplicaSetTopology(apps: number): ResourceTopology {
	const nodes: TopologyNode[] = [];
	const edges: ResourceTopology["edges"] = [];
	for (let index = 0; index < apps; index += 1) {
		const namespace = `namespace-${index % 100}`;
		const deployment = node("Deployment", `api-${index}`, namespace);
		const historical = node("ReplicaSet", `api-${index}-old`, namespace);
		historical.deploymentRevision = 1;
		const current = node("ReplicaSet", `api-${index}-current`, namespace);
		current.deploymentRevision = 2;
		const pod = node("Pod", `api-${index}-pod`, namespace);
		nodes.push(deployment, historical, current, pod);
		edges.push(
			{
				id: `owns-old-${index}`,
				source: deployment.id,
				target: historical.id,
				relation: "owns",
			},
			{
				id: `owns-current-${index}`,
				source: deployment.id,
				target: current.id,
				relation: "owns",
			},
			{
				id: `owns-pod-${index}`,
				source: current.id,
				target: pod.id,
				relation: "owns",
			},
		);
	}
	return { nodes, edges, warnings: [] };
}
