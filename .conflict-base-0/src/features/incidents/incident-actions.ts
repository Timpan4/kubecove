import type {
	IncidentCockpitItem,
	ResourceSummary,
	ResourceTopology,
} from "@/lib/types";

export type IncidentEnrichmentState = "idle" | "loading" | "ready" | "error";

export interface IncidentOwnerResolution {
	directOwner: ResourceSummary | null;
	workloadOwner: ResourceSummary | null;
	chain: ResourceSummary[];
	subjectFound?: boolean;
	complete?: boolean;
}

export interface IncidentAvailableAction {
	id: string;
	label: string;
	description: string;
	target: ResourceSummary;
	tone: "default" | "destructive";
}

const ACTIONABLE_WORKLOAD_KINDS = new Set(["Deployment", "StatefulSet", "DaemonSet"]);

function resourceIdentity(resource: ResourceSummary): string {
	return [
		resource.cluster,
		resource.kind,
		resource.namespace ?? "",
		resource.name,
	].join(":");
}

function resourceLabel(resource: ResourceSummary): string {
	return `${resource.kind}/${resource.name}`;
}

function emptyOwnerResolution(): IncidentOwnerResolution {
	return {
		directOwner: null,
		workloadOwner: null,
		chain: [],
		subjectFound: false,
		complete: false,
	};
}

export function resolveIncidentOwner(
	resource: ResourceSummary,
	topology: ResourceTopology | undefined,
): IncidentOwnerResolution {
	if (!topology) return emptyOwnerResolution();
	const selectedNode = topology.nodes.find(
		(node) => resourceIdentity(node.summary) === resourceIdentity(resource),
	);
	if (!selectedNode) return emptyOwnerResolution();

	const nodesById = new Map(topology.nodes.map((node) => [node.id, node]));
	const complete = topology.warnings.length === 0 && topology.edges.every(
		(edge) => edge.relation !== "owns" || (nodesById.has(edge.source) && nodesById.has(edge.target)),
	);
	const incoming = new Map<string, string[]>();
	for (const edge of topology.edges) {
		if (edge.relation !== "owns") continue;
		const sources = incoming.get(edge.target);
		if (sources) sources.push(edge.source);
		else incoming.set(edge.target, [edge.source]);
	}

	const queue: Array<{ nodeId: string; chain: ResourceSummary[] }> = [
		{ nodeId: selectedNode.id, chain: [] },
	];
	const visited = new Set([selectedNode.id]);
	let directOwner: ResourceSummary | null = null;
	let firstChain: ResourceSummary[] = [];

	while (queue.length > 0) {
		const current = queue.shift();
		if (!current) break;
		const owners = (incoming.get(current.nodeId) ?? [])
			.map((id) => nodesById.get(id))
			.filter((node) => node !== undefined)
			.sort((a, b) => resourceIdentity(a.summary).localeCompare(resourceIdentity(b.summary)));

		for (const ownerNode of owners) {
			if (visited.has(ownerNode.id)) continue;
			visited.add(ownerNode.id);
			const chain = [...current.chain, ownerNode.summary];
			directOwner ??= ownerNode.summary;
			if (firstChain.length === 0) firstChain = chain;
			if (ACTIONABLE_WORKLOAD_KINDS.has(ownerNode.summary.kind)) {
				return {
					directOwner,
					workloadOwner: ownerNode.summary,
					chain,
					subjectFound: true,
					complete,
				};
			}
			queue.push({ nodeId: ownerNode.id, chain });
		}
	}

	return {
		directOwner,
		workloadOwner: null,
		chain: firstChain,
		subjectFound: true,
		complete,
	};
}

function workloadActions(
	resource: ResourceSummary,
	prefix: "selected" | "owner",
): IncidentAvailableAction[] {
	const subject = prefix === "owner" ? `owning ${resource.kind}` : `this ${resource.kind}`;
	const actions: IncidentAvailableAction[] = [];
	if (resource.kind === "Deployment" || resource.kind === "StatefulSet") {
		actions.push(
			{
				id: `${prefix}:restart`,
				label: `Restart ${subject}`,
				description: `Open guarded rollout restart for ${resourceLabel(resource)}.`,
				target: resource,
				tone: "default",
			},
			{
				id: `${prefix}:scale`,
				label: `Scale ${subject}`,
				description: `Open guarded replica controls for ${resourceLabel(resource)}.`,
				target: resource,
				tone: "default",
			},
		);
	} else if (resource.kind === "DaemonSet") {
		actions.push({
			id: `${prefix}:restart`,
			label: `Restart ${subject}`,
			description: `Open guarded rollout restart for ${resourceLabel(resource)}.`,
			target: resource,
			tone: "default",
		});
	}
	return actions;
}

export function buildIncidentAvailableActions(
	item: IncidentCockpitItem,
	ownerResolution: IncidentOwnerResolution,
	topologyState: IncidentEnrichmentState,
): IncidentAvailableAction[] {
	if (item.resource.kind !== "Pod") {
		return workloadActions(item.resource, "selected");
	}

	const ownershipConfirmed = topologyState === "ready" && ownerResolution.complete;
	const workloadOwner = ownershipConfirmed ? ownerResolution.workloadOwner : null;
	const directOwner = ownershipConfirmed ? ownerResolution.directOwner : null;
	const controlled = Boolean(workloadOwner);
	const ownershipNote = workloadOwner
		? `${resourceLabel(workloadOwner)} should replace the deleted Pod.`
		: ownershipConfirmed && ownerResolution.subjectFound && !directOwner
			? "No controller owner was resolved; it will not be recreated automatically."
			: directOwner
				? `${resourceLabel(directOwner)} is not a supported restart or scale target; automatic replacement is not confirmed.`
				: "Controller ownership is unavailable; automatic replacement is not confirmed.";
	const actions: IncidentAvailableAction[] = [
		{
			id: "pod:delete",
			label: controlled ? "Recreate this Pod" : "Delete this Pod",
			description: `Open guarded delete preview for ${resourceLabel(item.resource)}. ${ownershipNote}`,
			target: item.resource,
			tone: "destructive",
		},
	];
	if (workloadOwner) actions.push(...workloadActions(workloadOwner, "owner"));
	return actions;
}
