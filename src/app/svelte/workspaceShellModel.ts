import {
	ARGO_NAV_KINDS,
	ARGO_PROVIDER_GROUP_ID,
	FLUX_FAMILIES,
	FLUX_PROVIDER_GROUP_ID,
} from "@/features/gitops/gitops-nav";
import { buildShallowNamespaceTreeNode } from "@/components/sidebar-tree-helpers";
import {
	discoveredResourceKindKey,
	resolveTreeScope,
	SECTIONS,
	type SectionName,
	type TreeNode,
	type TreeNodeId,
} from "@/lib/tree-nav";
import type {
	DiscoveredResourceKind,
	FluxDetectionSummary,
	NamespaceSummary,
	ResourceKindSelection,
	ResourceSummary,
} from "@/lib/types";
import { SUPPORTED_KINDS } from "@/lib/types";
import type { SavedWorkspace } from "@/lib/workspace-model";

export type WorkspaceViewMode =
	| "overview"
	| "resources"
	| "argo"
	| "helm"
	| "incidents"
	| "portForwards"
	| "rbac"
	| "settings";

export const DEFAULT_WORKSPACE_VIEW: WorkspaceViewMode =
	"overview";

const SECTION_LABELS: Record<string, string> = {
	workspaceOverview: "Workspace Overview",
	clusterOverview: "Cluster Overview",
	namespaces: "Namespaces",
	workloads: "Workloads",
	network: "Network",
	config: "Config",
	storage: "Storage",
	discovered: "Custom Resources",
	argo: "GitOps",
	helm: "Helm",
	incidents: "Incidents",
	portForwards: "Port Forwards",
	rbac: "RBAC",
};

export const GITOPS_RESOURCE_KINDS: ResourceKindSelection[] = [
	...SUPPORTED_KINDS,
	...SECTIONS.clusterOverview.children,
	"CustomResourceDefinition",
];

function resourceKindLabel(kind: ResourceKindSelection): string {
	return typeof kind === "string" ? kind : kind.kind;
}

function resourceKindSelectionKey(kind: ResourceKindSelection): string {
	return typeof kind === "string"
		? `typed:${kind}`
		: `dynamic:${discoveredResourceKindKey(kind)}`;
}

function gitOpsGroupLabel(group: string | undefined): string | null {
	if (group === ARGO_PROVIDER_GROUP_ID) return "Argo CD";
	if (group === FLUX_PROVIDER_GROUP_ID) return "Flux";
	return FLUX_FAMILIES.find((family) => family.groupId === group)?.label ?? null;
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

export function viewModeForTreeNode(
	nodeId: TreeNodeId | null,
): WorkspaceViewMode {
	if (!nodeId) return "resources";
	if (nodeId.type === "section" && nodeId.section === "workspaceOverview") {
		return "overview";
	}
	if (nodeId.section === "argo") return "argo";
	if (nodeId.section === "helm") return "helm";
	if (nodeId.section === "rbac") return "rbac";
	const scope = resolveTreeScope(nodeId);
	if (scope.argoMode) return "argo";
	if (scope.helmMode) return "helm";
	if (scope.incidentMode) return "incidents";
	if (scope.portForwardMode) return "portForwards";
	if (scope.rbacMode) return "rbac";
	return "resources";
}

export function treeNodeForResource(resource: ResourceSummary): TreeNodeId {
	return {
		type: "kind",
		section: sectionForKind(resource.kind),
		namespace: resource.namespace ?? undefined,
		kind: resource.kind,
	};
}

function sectionForKind(kind: string): SectionName {
	for (const [section, config] of Object.entries(SECTIONS) as Array<
		[SectionName, (typeof SECTIONS)[SectionName]]
	>) {
		if ((config.children as readonly string[]).includes(kind)) return section;
	}
	return "discovered";
}

export function isNamespaceListView({
	selectedNode,
	viewMode,
}: {
	selectedNode: TreeNodeId | null;
	viewMode: WorkspaceViewMode;
}): boolean {
	return (
		viewMode === "resources" &&
		selectedNode?.type === "section" &&
		selectedNode.section === "namespaces"
	);
}

export interface ResourceBrowserScope {
	canQuery: boolean;
	namespaces: string[];
	kinds: ResourceKindSelection[];
}

export function getResourceBrowserScope({
	workspace,
	selectedNode,
	viewMode,
}: {
	workspace: SavedWorkspace;
	selectedNode: TreeNodeId | null;
	viewMode: WorkspaceViewMode;
}): ResourceBrowserScope {
	if (viewMode !== "resources" || isNamespaceListView({ selectedNode, viewMode })) {
		return { canQuery: false, namespaces: [], kinds: [] };
	}
	const scope = resolveTreeScope(selectedNode);
	if (scope.argoMode || scope.helmMode || scope.incidentMode || scope.portForwardMode || scope.rbacMode) {
		return { canQuery: false, namespaces: [], kinds: [] };
	}
	const kinds =
		scope.kinds.length > 0 ? scope.kinds : [...workspace.scope.kinds];
	const namespaces = scope.namespace
		? [scope.namespace]
		: scope.section === "namespaces" || scope.clusterScoped
			? []
			: [...workspace.scope.namespaces];
	return { canQuery: kinds.length > 0, namespaces, kinds };
}

export function getWorkspaceTitle({
	workspace,
	selectedNode,
	viewMode,
}: {
	workspace: SavedWorkspace;
	selectedNode: TreeNodeId | null;
	viewMode: WorkspaceViewMode;
}): string {
	const scope = resolveTreeScope(selectedNode);
	if (viewMode === "overview") return workspace.name;
	if (viewMode === "settings") return "Settings";
	if (viewMode === "helm") return "Helm Releases";
	if (viewMode === "incidents") return "Incident Cockpit";
	if (viewMode === "portForwards") return "Port Forwards";
	if (viewMode === "rbac") {
		return selectedNode?.type === "kind" && selectedNode.kind
			? selectedNode.kind
			: "RBAC";
	}
	if (viewMode === "argo") {
		if (selectedNode?.type === "group") {
			return gitOpsGroupLabel(selectedNode.group) ?? "GitOps";
		}
		return selectedNode?.type === "kind" && selectedNode.kind
			? selectedNode.kind
			: "GitOps";
	}
	if (!scope.section) return "Kubernetes Resources";
	if (scope.section === "clusterOverview") {
		if (scope.kinds.length === 1) {
			return `${resourceKindLabel(scope.kinds[0])} Resources`;
		}
		return "Cluster Overview";
	}
	if (scope.section === "namespaces" && scope.namespace) {
		return scope.group ? `${scope.namespace} / ${scope.group}` : scope.namespace;
	}
	if (scope.group) return scope.group;
	if (scope.kinds.length === 1) {
		return `${resourceKindLabel(scope.kinds[0])} Resources`;
	}
	return SECTION_LABELS[scope.section] ?? scope.section;
}

export function getWorkspacePlaceholder({
	selectedNode,
	viewMode,
}: {
	selectedNode: TreeNodeId | null;
	viewMode: WorkspaceViewMode;
}): string {
	if (viewMode === "overview") {
		return "Use the sidebar to open resource and app surfaces for this workspace.";
	}
	if (viewMode === "settings") {
		return "Adjust runtime, safety, and workspace preferences.";
	}
	const scope = resolveTreeScope(selectedNode);
	if (scope.kinds.length > 0) {
		return "Browse live resources for this scope.";
	}
	if (scope.argoMode) return "Inspect GitOps applications and sources.";
	if (scope.helmMode) return "Inspect Helm releases, values, and manifests.";
	if (scope.incidentMode) return "Review incident signals and timelines.";
	if (scope.portForwardMode) {
		return "Review saved and active live sessions.";
	}
	if (scope.rbacMode) return "Inspect RBAC bindings, subjects, and permissions.";
	return "Select a sidebar item to open a workspace surface.";
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
