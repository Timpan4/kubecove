import type {
	DiscoveredResourceKind,
	ResourceKindSelection,
	ResourceSummary,
	ResourceTopology,
	TopologyNode,
} from "@/lib/types";
import { kindKey, kindMatcher } from "@/lib/kind-identity";
import { findResourceIndex, looseResourceKey } from "@/lib/resource-identity";
import { CLUSTER_SCOPED_KINDS, SUPPORTED_KINDS } from "@/lib/types";

export function initialOwnershipMapOpen(
	restoredState: { mapPanelOpen: boolean } | null | undefined,
	showByDefault: boolean,
): boolean {
	return restoredState?.mapPanelOpen ?? showByDefault;
}

export function shouldLoadOwnershipMap(
	mapPanelOpen: boolean,
	componentLoaded: boolean,
	loadFailed: boolean,
): boolean {
	return mapPanelOpen && !componentLoaded && !loadFailed;
}

/** Available kinds that were not available before and are not selected yet. */
export function newlyAvailableKinds(
	available: ResourceKindSelection[],
	previous: ResourceKindSelection[],
	selected: ResourceKindSelection[],
): ResourceKindSelection[] {
	const known = new Set([...previous, ...selected].map(kindKey));
	return available.filter((kind) => !known.has(kindKey(kind)));
}

export interface ResourceScopeOption {
	key: string;
	label: string;
}

export function filterResourceScopeOptions(
	options: ResourceScopeOption[],
	search: string,
): ResourceScopeOption[] {
	const query = search.trim().toLowerCase();
	return query
		? options.filter((option) => option.label.toLowerCase().includes(query))
		: options;
}

export function nextNamespaceSelection(
	selectedNamespaces: string[],
	availableNamespaces: string[],
	namespace: string,
	checked: boolean,
): string[] {
	if (checked) return Array.from(new Set([...selectedNamespaces, namespace])).sort();
	const currentSelection = selectedNamespaces.length > 0 ? selectedNamespaces : availableNamespaces;
	return currentSelection.filter((item) => item !== namespace);
}

export function allKindOptions(
	discoveredKinds: DiscoveredResourceKind[],
): ResourceKindSelection[] {
	const discovered = [...discoveredKinds]
		.sort((left, right) => left.kind.localeCompare(right.kind))
	return [...SUPPORTED_KINDS, ...CLUSTER_SCOPED_KINDS, ...discovered];
}

export function filterResourcesByKinds(
	resources: ResourceSummary[],
	kinds: ResourceKindSelection[],
): ResourceSummary[] {
	return resources.filter(kindMatcher(kinds));
}

export function filterTopologyByKinds(
	topology: ResourceTopology | undefined,
	kinds: ResourceKindSelection[],
): ResourceTopology | undefined {
	if (!topology) return undefined;
	const matches = kindMatcher(kinds);
	const nodes = topology.nodes.filter((node) => matches(node.summary));
	const nodeIds = new Set(nodes.map((node) => node.id));
	return {
		...topology,
		nodes,
		edges: topology.edges.filter(
			(edge) => nodeIds.has(edge.source) && nodeIds.has(edge.target),
		),
	};
}

/**
 * The kinds the map links from: the selection, when it leaves out one of the
 * default kinds. Null keeps the map to table rows and their owners. Custom
 * kinds added beyond the defaults do not narrow the selection.
 */
export function topologyLinkKinds(
	selected: ResourceKindSelection[],
	defaults: ResourceKindSelection[],
): ResourceKindSelection[] | null {
	const keys = new Set(selected.map(kindKey));
	return defaults.some((kind) => !keys.has(kindKey(kind))) ? selected : null;
}

/**
 * Keeps matching resources and their ownership explanation. With `linkKinds`,
 * a narrowed kind selection, it keeps matching resources and the nodes one edge
 * from them instead. A linked node of a selected kind that is not a row was
 * filtered out of the table, so it stays out of the map too.
 */
export function filterTopologyByTableRows(
	topology: ResourceTopology | undefined,
	rows: ResourceSummary[],
	linkKinds: ResourceKindSelection[] | null = null,
): ResourceTopology | undefined {
	if (!topology) return undefined;
	const rowKeys = new Set(rows.map(looseResourceKey));
	const nodesById = new Map(topology.nodes.map((node) => [node.id, node]));
	const matched = new Set(
		topology.nodes
			.filter((node) => rowKeys.has(looseResourceKey(node.summary)))
			.map((node) => node.id),
	);
	const nodeIds = new Set(matched);
	if (linkKinds) {
		const selected = kindMatcher(linkKinds);
		const link = (id: string) => {
			const node = nodesById.get(id);
			if (node && !selected(node.summary)) nodeIds.add(id);
		};
		for (const edge of topology.edges) {
			if (matched.has(edge.source)) link(edge.target);
			if (matched.has(edge.target)) link(edge.source);
		}
	} else {
		const incoming = new Map<string, string[]>();
		for (const edge of topology.edges) {
			const sources = incoming.get(edge.target) ?? [];
			sources.push(edge.source);
			incoming.set(edge.target, sources);
		}
		const pending = [...nodeIds];
		for (let index = 0; index < pending.length; index += 1) {
			for (const parentId of incoming.get(pending[index]) ?? []) {
				if (nodeIds.has(parentId) || !nodesById.has(parentId)) continue;
				nodeIds.add(parentId);
				pending.push(parentId);
			}
		}
	}
	return {
		...topology,
		nodes: topology.nodes.filter((node) => nodeIds.has(node.id)),
		edges: topology.edges.filter((edge) => nodeIds.has(edge.source) && nodeIds.has(edge.target)),
	};
}

export function createTableTopologyFilter() {
	let source: ResourceTopology | undefined;
	let hideHistorical = false;
	let keys = new Set<string>();
	let links: string | null = null;
	let result: ResourceTopology | undefined;
	return (
		topology: ResourceTopology | undefined,
		rows: ResourceSummary[],
		hide: boolean,
		linkKinds: ResourceKindSelection[] | null = null,
	) => {
		const nextKeys = new Set<string>();
		for (const row of rows) nextKeys.add(looseResourceKey(row));
		let sameKeys = nextKeys.size === keys.size;
		for (const key of nextKeys) {
			if (!keys.has(key)) { sameKeys = false; break; }
		}
		const nextLinks = linkKinds ? linkKinds.map(kindKey).sort().join("\n") : null;
		if (topology === source && hide === hideHistorical && sameKeys && nextLinks === links) return result;
		source = topology;
		hideHistorical = hide;
		keys = nextKeys;
		links = nextLinks;
		result = filterTopologyByTableRows(filterHistoricalReplicaSets(topology, hide), rows, linkKinds);
		return result;
	};
}

export function filterHistoricalReplicaSets(
	topology: ResourceTopology | undefined,
	hide: boolean,
): ResourceTopology | undefined {
	if (!topology || !hide) return topology;
	const incoming = new Map<string, TopologyNode[]>();
	const nodesById = new Map(topology.nodes.map((node) => [node.id, node]));
	const podOwners = new Set<string>();
	for (const edge of topology.edges) {
		const source = nodesById.get(edge.source);
		const target = nodesById.get(edge.target);
		if (source && target) {
			const parents = incoming.get(target.id) ?? [];
			parents.push(source);
			incoming.set(target.id, parents);
			if (source.kind === "ReplicaSet" && target.kind === "Pod") podOwners.add(source.id);
		}
	}
	const hidden = new Set(
		topology.nodes
			.filter((node) => {
				if (node.kind !== "ReplicaSet" || podOwners.has(node.id) || node.deploymentRevision === undefined) return false;
				const deployment = incoming.get(node.id)?.find((parent) => parent.kind === "Deployment");
				return deployment?.deploymentRevision !== undefined && node.deploymentRevision < deployment.deploymentRevision;
			})
			.map((node) => node.id),
	);
	return {
		...topology,
		nodes: topology.nodes.filter((node) => !hidden.has(node.id)),
		edges: topology.edges.filter((edge) => !hidden.has(edge.source) && !hidden.has(edge.target)),
	};
}

export function syncedTopologyNodeId({
	selectedTopologyNodeId,
	selectedResource,
	topologyNodes,
}: {
	selectedTopologyNodeId: string | null;
	selectedResource: ResourceSummary | null;
	topologyNodes: TopologyNode[] | undefined;
}): string | null {
	if (!topologyNodes) return selectedTopologyNodeId;
	if (
		selectedTopologyNodeId &&
		!topologyNodes.some((node) => node.id === selectedTopologyNodeId)
	) {
		return null;
	}
	if (!selectedResource) return selectedTopologyNodeId;
	const selectedFromTable =
		topologyNodes[findResourceIndex(topologyNodes, selectedResource, (node) => node.summary)];
	return selectedFromTable?.id ?? selectedTopologyNodeId;
}
