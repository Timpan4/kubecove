import {
	ARGO_NAV_KINDS,
	ARGO_PROVIDER_GROUP_ID,
	FLUX_FAMILIES,
	FLUX_PROVIDER_GROUP_ID,
} from "@/features/gitops/gitops-nav";
import { buildShallowNamespaceTreeNode } from "@/components/sidebar-tree-helpers";
import {
	discoveredResourceKindKey,
	SECTIONS,
	type TreeNode,
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

function resourceKindSelectionKey(kind: ResourceKindSelection): string {
	return typeof kind === "string"
		? `typed:${kind}`
		: `dynamic:${discoveredResourceKindKey(kind)}`;
}

export function extraDiscoveredKinds(
	resourceKinds: DiscoveredResourceKind[],
): DiscoveredResourceKind[] {
	return resourceKinds
		.toSorted((left, right) => {
			return (
				left.kind.localeCompare(right.kind) ||
				left.apiVersion.localeCompare(right.apiVersion) ||
				left.plural.localeCompare(right.plural)
			);
		});
}

export function appendPresentCustomResourceKinds(
	kinds: ResourceKindSelection[],
	customResourceKinds: DiscoveredResourceKind[],
): ResourceKindSelection[] {
	const seen = new Set(kinds.map(resourceKindSelectionKey));
	return kinds.concat(
		customResourceKinds.filter((kind) => {
			const key = resourceKindSelectionKey(kind);
			if (seen.has(key)) return false;
			seen.add(key);
			return true;
		}),
	);
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
}): TreeNode[] {
	const extraKinds = extraDiscoveredKinds(resourceKinds);
	const namespaceNode: TreeNode = {
		id: { type: "section", section: "namespaces" },
		label: SECTIONS.namespaces.label,
		children: namespaces.map((namespace) =>
			buildShallowNamespaceTreeNode(namespace.name),
		),
	};
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
				? extraKinds.map((resourceKind) => ({
						id: {
							type: "kind",
							section: "discovered",
							kind: discoveredResourceKindKey(resourceKind),
							resourceKind,
						},
						label: resourceKind.kind,
						description: `${resourceKind.apiVersion} / ${resourceKind.plural} / ${
							resourceKind.namespaced ? "namespaced" : "cluster-scoped"
						}`,
					}))
				: [
						{
							id: { type: "kind", section: "discovered", kind: "__empty" },
							label: "No custom resources",
							disabled: true,
						},
					];

	return [
		{
			id: { type: "section", section: "workspaceOverview" },
			label: SECTIONS.workspaceOverview.label,
		},
		{
			id: { type: "section", section: "clusterOverview" },
			label: SECTIONS.clusterOverview.label,
			children: SECTIONS.clusterOverview.children.map((kind) => ({
				id: { type: "kind", section: "clusterOverview", kind },
				label: kind,
			})),
		},
		namespaceNode,
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
		{ id: { type: "section", section: "incidents" }, label: SECTIONS.incidents.label },
		{
			id: { type: "section", section: "portForwards" },
			label: SECTIONS.portForwards.label,
		},
		{
			id: { type: "section", section: "rbac" },
			label: SECTIONS.rbac.label,
			children: SECTIONS.rbac.children.map((kind) => ({
				id: { type: "kind", section: "rbac", kind },
				label: kind,
			})),
		},
	];
}
