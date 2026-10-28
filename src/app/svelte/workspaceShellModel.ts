import {
	buildCustomResourceGroupNodes,
	buildShallowNamespaceTreeNode,
	extraDiscoveredKinds,
	filterCustomResourceKinds,
} from "@/components/sidebar-tree-helpers";
import {
	ARGO_NAV_KINDS,
	ARGO_PROVIDER_GROUP_ID,
	FLUX_FAMILIES,
	FLUX_PROVIDER_GROUP_ID,
} from "@/features/gitops/gitops-nav";
import { uniqueKinds } from "@/lib/kind-identity";
import {
	nodeIdToString,
	SECTIONS,
	type TreeNode,
	type TreeNodeId,
} from "@/lib/tree-nav";
import type {
	DiscoveredResourceKind,
	FluxDetectionSummary,
	NamespaceSummary,
	ResourceKindSelection,
} from "@/lib/types";
import { SUPPORTED_KINDS } from "@/lib/types";

export const GITOPS_RESOURCE_KINDS: ResourceKindSelection[] = [
	...SUPPORTED_KINDS,
	...SECTIONS.clusterOverview.children,
	"CustomResourceDefinition",
];

const COMPACT_RESOURCE_SECTION_IDS = new Set(
	(
		[
			"clusterOverview",
			"workloads",
			"network",
			"config",
			"storage",
			"discovered",
		] as const
	).map(
		(section) => nodeIdToString({ type: "section", section }),
	),
);

export function toggleCompactSidebarSection(
	expandedSections: string[],
	id: string,
): string[] {
	if (expandedSections.includes(id)) {
		return expandedSections.filter((item) => item !== id);
	}
	if (!COMPACT_RESOURCE_SECTION_IDS.has(id)) return [...expandedSections, id];
	return expandedSections
		.filter((item) => !COMPACT_RESOURCE_SECTION_IDS.has(item))
		.concat(id);
}

export function resourceBrowserAvailableKinds(
	kinds: ResourceKindSelection[],
	customResourceKinds: DiscoveredResourceKind[],
	fixedKindScope: boolean,
): ResourceKindSelection[] {
	return fixedKindScope ? [...kinds] : uniqueKinds([...kinds, ...customResourceKinds]);
}

function buildArgoProviderNode(disabled: boolean): TreeNode {
	return {
		id: { type: "group", section: "argo", group: ARGO_PROVIDER_GROUP_ID },
		label: "Argo CD",
		disabled,
		description: disabled
			? "Argo CD CRDs were not detected in this cluster."
			: undefined,
		children: disabled
			? []
			: ARGO_NAV_KINDS.map((kind) => ({
					id: {
						type: "kind",
						section: "argo",
						group: ARGO_PROVIDER_GROUP_ID,
						kind: kind.label,
					},
					label: kind.label,
				})),
	};
}

function buildFluxProviderNode(disabled: boolean): TreeNode {
	return {
		id: { type: "group", section: "argo", group: FLUX_PROVIDER_GROUP_ID },
		label: "Flux",
		disabled,
		description: disabled ? "Flux CRDs were not detected in this cluster." : undefined,
		children: disabled
			? []
			: FLUX_FAMILIES.map((family) => ({
					id: { type: "group", section: "argo", group: family.groupId },
					label: family.label,
					children: family.kinds.map((kind) => ({
						id: {
							type: "kind",
							section: "argo",
							group: family.groupId,
							kind: kind.label,
						},
						label: kind.label,
					})),
				})),
	};
}

function buildGitOpsNode({
	argoDetected,
	fluxDetection,
	detecting,
	showUnavailableGitOpsProviders,
}: {
	argoDetected: boolean | undefined;
	fluxDetection: FluxDetectionSummary | undefined;
	detecting: boolean;
	showUnavailableGitOpsProviders: boolean;
}): TreeNode {
	const children: TreeNode[] = [];
	const fluxDetected = fluxDetection?.detected === true;
	if (argoDetected || (showUnavailableGitOpsProviders && argoDetected === false)) {
		children.push(buildArgoProviderNode(!argoDetected));
	}
	if (
		fluxDetected ||
		(showUnavailableGitOpsProviders && fluxDetection?.detected === false)
	) {
		children.push(buildFluxProviderNode(!fluxDetected));
	}
	if (children.length === 0 && detecting) {
		children.push({
			id: { type: "group", section: "argo", group: "gitops:detecting" },
			label: "Detecting providers...",
			disabled: true,
		});
	}
	return {
		id: { type: "section", section: "argo" },
		label: SECTIONS.argo.label,
		children,
	};
}

export interface SidebarTreeGroup {
	id: "workspace" | "cluster" | "scope" | "namespaces";
	nodes: TreeNode[];
}

export function buildSidebarTree({
	namespaces,
	resourceKinds,
	argoDetected,
	fluxDetection,
	detectingGitOps,
	resourceKindsPending,
	resourceKindsError,
	showUnavailableGitOpsProviders,
	showCustomResources = true,
	customResourceSearch = "",
}: {
	namespaces: NamespaceSummary[];
	resourceKinds: DiscoveredResourceKind[];
	argoDetected: boolean | undefined;
	fluxDetection: FluxDetectionSummary | undefined;
	detectingGitOps: boolean;
	resourceKindsPending: boolean;
	resourceKindsError: string;
	showUnavailableGitOpsProviders: boolean;
	showCustomResources?: boolean;
	customResourceSearch?: string;
}): SidebarTreeGroup[] {
	const extraKinds = extraDiscoveredKinds(
		filterCustomResourceKinds(resourceKinds, customResourceSearch),
	);
	const discoveredChildren: TreeNode[] = resourceKindsPending
		? [
				{
					id: { type: "kind", section: "discovered", kind: "__loading" },
					label: "Loading custom resources...",
					disabled: true,
				},
			]
		: resourceKindsError
			? [
					{
						id: { type: "kind", section: "discovered", kind: "__error" },
						label: "Custom resource discovery unavailable",
						description: resourceKindsError,
						disabled: true,
					},
				]
			: extraKinds.length > 0
				? buildCustomResourceGroupNodes({ section: "discovered", extraKinds })
				: [
						{
							id: { type: "kind", section: "discovered", kind: "__empty" },
							label: customResourceSearch.trim()
								? "No custom resources match search"
								: "No custom resources",
							disabled: true,
						},
					];

	const workspaceNodes: TreeNode[] = [
		{
			id: { type: "section", section: "workspaceOverview" },
			label: SECTIONS.workspaceOverview.label,
		},
		{ id: { type: "section", section: "incidents" }, label: SECTIONS.incidents.label },
		{
			id: { type: "section", section: "portForwards" },
			label: SECTIONS.portForwards.label,
		},
	];
	const clusterNodes: TreeNode[] = [
		buildGitOpsNode({
			argoDetected,
			fluxDetection,
			detecting: detectingGitOps,
			showUnavailableGitOpsProviders,
		}),
		{
			id: { type: "section", section: "helm" },
			label: SECTIONS.helm.label,
			children: SECTIONS.helm.children.map((kind) => ({
				id: { type: "kind", section: "helm", kind },
				label: kind,
			})),
		},
		{
			id: { type: "section", section: "rbac" },
			label: SECTIONS.rbac.label,
			children: SECTIONS.rbac.children.map((kind) => ({
				id: { type: "kind", section: "rbac", kind },
				label: kind,
			})),
		},
		{
			id: { type: "section", section: "clusterOverview" },
			label: SECTIONS.clusterOverview.label,
			children: SECTIONS.clusterOverview.children.map((kind) => ({
				id: { type: "kind", section: "clusterOverview", kind },
				label: kind,
			})),
		},
	];
	const scopeNodes: TreeNode[] = [
		...(["workloads", "network", "config", "storage"] as const).map(
			(section) => ({
				id: { type: "section" as const, section },
				label: SECTIONS[section].label,
				children: SECTIONS[section].children.map((kind) => ({
					id: { type: "kind" as const, section, kind },
					label: kind,
				})),
			}),
		),
		...(showCustomResources
			? [
					{
						id: { type: "section" as const, section: "discovered" as const },
						label: SECTIONS.discovered.label,
						children: discoveredChildren,
					},
				]
			: []),
	];
	const namespaceNodes = namespaces.map((namespace) =>
		buildShallowNamespaceTreeNode(namespace.name),
	);
	return [
		{ id: "workspace", nodes: workspaceNodes },
		{ id: "cluster", nodes: clusterNodes },
		{ id: "scope", nodes: scopeNodes },
		{ id: "namespaces", nodes: namespaceNodes },
	];
}

export function sidebarSelectionNode(
	node: TreeNodeId | null,
	groups: SidebarTreeGroup[] = [],
): TreeNodeId | null {
	if (node?.type !== "kind" || node.section === "namespaces") return node;
	const selection = node.namespace
		? { ...node, namespace: undefined }
		: node;
	const matches: TreeNodeId[] = [];
	const visit = (nodes: TreeNode[]) => {
		for (const candidate of nodes) {
			const id = candidate.id;
			if (!candidate.disabled && id.type === "kind" && id.section === selection.section &&
				(!selection.group || id.group === selection.group) &&
				(id.kind === selection.kind || id.resourceKind?.kind === selection.kind)) {
				matches.push(id);
			}
			if (candidate.children) visit(candidate.children);
		}
	};
	for (const group of groups) visit(group.nodes);
	return matches.find((id) => nodeIdToString(id) === nodeIdToString(selection)) ??
		(matches.length === 1 ? matches[0] :
			groups.flatMap((group) => group.nodes).find((candidate) =>
				candidate.id.type === "section" && candidate.id.section === selection.section,
			)?.id ?? selection);
}

export function collectSelectedAncestorIds(
	groups: SidebarTreeGroup[],
	selectedNode: TreeNodeId | null,
	getLazyChildren?: (node: TreeNode) => TreeNode[] | undefined,
): Set<string> {
	if (!selectedNode) return new Set<string>();
	const selectedId = nodeIdToString(selectedNode);
	const findAncestorIds = (nodes: TreeNode[]): string[] | null => {
		for (const node of nodes) {
			const id = nodeIdToString(node.id);
			if (id === selectedId) return [];
			const childAncestorIds = findAncestorIds(
				getLazyChildren?.(node) ?? node.children ?? [],
			);
			if (childAncestorIds) return [id, ...childAncestorIds];
			// A folded namespace's custom-resource children may be unloaded.
			if (node.id.type === "namespace" && selectedNode.section === "namespaces" &&
				node.id.namespace === selectedNode.namespace) return [id];
		}
		return null;
	};
	for (const group of groups) {
		const ancestorIds = findAncestorIds(group.nodes);
		if (ancestorIds) return new Set(ancestorIds);
	}
	return new Set<string>();
}
