import {
	ARGO_PROVIDER_GROUP_ID,
	FLUX_FAMILIES,
	FLUX_PROVIDER_GROUP_ID,
} from "@/features/gitops/gitops-nav";
import type { ArgoApplicationSummary } from "@/lib/gitops-types";
import {
	resourceRefFromSummary,
	resourceSummaryFromRef,
	type PathStateHealthFilter,
	type PathStateIncidentFilter,
	type PathStateResourceBrowserState,
	type PathStateResourceDetailState,
	type PathStateSurfacesState,
	type PathStateWorkspaceSnapshot,
	type PathStateWorkspaceViewMode,
} from "@/lib/path-state";
import {
	resolveTreeScope,
	SECTIONS,
	type SectionName,
	type TreeNodeId,
} from "@/lib/tree-nav";
import type { ResourceKindSelection, ResourceSummary } from "@/lib/types";
import {
	resourceKindLabel,
	type SavedWorkspace,
} from "@/lib/workspace-model";

export interface ResourceBrowserScope {
	canQuery: boolean;
	namespaces: string[];
	kinds: ResourceKindSelection[];
}

export interface WorkspaceNavigationModel {
	activeSurface: WorkspaceViewMode;
	title: string;
	placeholder: string;
	isOverview: boolean;
	isNamespaceList: boolean;
	resourceBrowserScope: ResourceBrowserScope;
}

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

function gitOpsGroupLabel(group: string | undefined): string | null {
	if (group === ARGO_PROVIDER_GROUP_ID) return "Argo CD";
	if (group === FLUX_PROVIDER_GROUP_ID) return "Flux";
	return FLUX_FAMILIES.find((family) => family.groupId === group)?.label ?? null;
}

function isNamespaceListView(state: WorkspaceNavigationState): boolean {
	return (
		state.viewMode === "resources" &&
		state.selectedNode?.type === "section" &&
		state.selectedNode.section === "namespaces"
	);
}

function resourceBrowserScope(
	workspace: SavedWorkspace,
	state: WorkspaceNavigationState,
): ResourceBrowserScope {
	if (state.viewMode !== "resources" || isNamespaceListView(state)) {
		return { canQuery: false, namespaces: [], kinds: [] };
	}
	const scope = resolveTreeScope(state.selectedNode);
	if (
		scope.argoMode ||
		scope.helmMode ||
		scope.incidentMode ||
		scope.portForwardMode ||
		scope.rbacMode
	) {
		return { canQuery: false, namespaces: [], kinds: [] };
	}
	const kinds =
		scope.kinds.length > 0 ? scope.kinds : [...workspace.scope.kinds];
	const namespaces = state.resourceNamespaceOverride ??
		(scope.namespace
			? [scope.namespace]
			: scope.section === "namespaces" || scope.clusterScoped
				? []
				: [...workspace.scope.namespaces]);
	return { canQuery: kinds.length > 0, namespaces, kinds };
}

function workspaceTitle(
	workspace: SavedWorkspace,
	state: WorkspaceNavigationState,
): string {
	const scope = resolveTreeScope(state.selectedNode);
	if (state.viewMode === "overview") return workspace.name;
	if (state.viewMode === "settings") return "Settings";
	if (state.viewMode === "helm") return "Helm Releases";
	if (state.viewMode === "incidents") return "Incident Cockpit";
	if (state.viewMode === "portForwards") return "Port Forwards";
	if (state.viewMode === "rbac") {
		return state.selectedNode?.type === "kind" && state.selectedNode.kind
			? state.selectedNode.kind
			: "RBAC";
	}
	if (state.viewMode === "argo") {
		if (state.selectedNode?.type === "group") {
			return gitOpsGroupLabel(state.selectedNode.group) ?? "GitOps";
		}
		return state.selectedNode?.type === "kind" && state.selectedNode.kind
			? state.selectedNode.kind
			: "GitOps";
	}
	if (!scope.section) return "Kubernetes Resources";
	if (scope.section === "clusterOverview") {
		return scope.kinds.length === 1
			? `${resourceKindLabel(scope.kinds[0])} Resources`
			: "Cluster Overview";
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

function workspacePlaceholder(state: WorkspaceNavigationState): string {
	if (state.viewMode === "overview") {
		return "Use the sidebar to open resource and app surfaces for this workspace.";
	}
	if (state.viewMode === "settings") {
		return "Adjust runtime, safety, and workspace preferences.";
	}
	const scope = resolveTreeScope(state.selectedNode);
	if (scope.kinds.length > 0) return "Browse live resources for this scope.";
	if (scope.argoMode) return "Inspect GitOps applications and sources.";
	if (scope.helmMode) return "Inspect Helm releases, values, and manifests.";
	if (scope.incidentMode) return "Review incident signals and timelines.";
	if (scope.portForwardMode) return "Review saved and active live sessions.";
	if (scope.rbacMode) return "Inspect RBAC bindings, subjects, and permissions.";
	return "Select a sidebar item to open a workspace surface.";
}

export function buildWorkspaceNavigationModel(
	workspace: SavedWorkspace,
	state: WorkspaceNavigationState,
): WorkspaceNavigationModel {
	return {
		activeSurface: state.viewMode,
		title: workspaceTitle(workspace, state),
		placeholder: workspacePlaceholder(state),
		isOverview: state.viewMode === "overview",
		isNamespaceList: isNamespaceListView(state),
		resourceBrowserScope: resourceBrowserScope(workspace, state),
	};
}

export type WorkspaceViewMode = PathStateWorkspaceViewMode;

export const DEFAULT_WORKSPACE_VIEW: WorkspaceViewMode = "overview";
export const DEFAULT_WORKSPACE_NODE: TreeNodeId = {
	type: "section",
	section: "workspaceOverview",
};

export interface WorkspaceNavigationState {
	selectedNode: TreeNodeId | null;
	viewMode: WorkspaceViewMode;
	focusedResource: ResourceSummary | null;
	restoreTargetResource: ResourceSummary | null;
	targetHelmRelease: { name: string; namespace?: string | null } | null;
	targetGitOpsApplication: string | null;
	resourceGitOpsFocusApplication: ArgoApplicationSummary | null;
	resourceInitialSearch: string;
	resourceInitialGitOpsFilter: string;
	resourceInitialHealthFilter: PathStateHealthFilter;
	resourceNamespaceOverride: string[] | null;
	resourceBrowserPathState: PathStateResourceBrowserState | null;
	initialIncidentFilter: PathStateIncidentFilter;
}

export type WorkspaceNavigationIntent =
	| { type: "openLauncher" }
	| { type: "changeCluster" }
	| { type: "openSettings" }
	| { type: "closeSettings" }
	| {
			type: "openResources";
			namespaces?: string | string[];
			search?: string;
			gitOpsFilter?: string;
			healthFilter?: PathStateHealthFilter;
			gitOpsFocusApplication?: ArgoApplicationSummary | null;
	  }
	| { type: "openHelmRelease"; name: string; namespace?: string | null }
	| { type: "openArgo"; application?: string }
	| { type: "openIncidents"; filter?: PathStateIncidentFilter }
	| { type: "openPortForwards" }
	| { type: "selectNode"; node: TreeNodeId }
	| { type: "selectResource"; resource: ResourceSummary; node: TreeNodeId }
	| { type: "focusResource"; resource: ResourceSummary }
	| { type: "inspectResource"; resource: ResourceSummary }
	| { type: "updateResourceBrowserPath"; pathState: PathStateResourceBrowserState | null }
	| { type: "clearResource" }
	| { type: "clearHelmTarget" }
	| { type: "clearGitOpsTarget" };

export function createWorkspaceNavigation(
	workspace: SavedWorkspace,
	snapshot: PathStateWorkspaceSnapshot | null = null,
): WorkspaceNavigationState {
	const restored = workspacePathForWorkspace(workspace, snapshot);
	const restoreRef = restored?.restoreTargetResource ?? restored?.focusedResource ?? null;
	return {
		selectedNode: restored ? restored.selectedNode : DEFAULT_WORKSPACE_NODE,
		viewMode: restored?.viewMode ?? DEFAULT_WORKSPACE_VIEW,
		focusedResource: null,
		restoreTargetResource: restoreRef ? resourceSummaryFromRef(restoreRef) : null,
		targetHelmRelease:
			restored?.targetHelmRelease ?? restored?.surfaces?.selectedHelmRelease ?? null,
		targetGitOpsApplication:
			restored?.targetGitOpsApplication ??
			restored?.surfaces?.selectedGitOpsApplication ??
			null,
		resourceGitOpsFocusApplication: null,
		resourceInitialSearch: restored?.resourceInitialSearch ?? "",
		resourceInitialGitOpsFilter: restored?.resourceInitialGitOpsFilter ?? "",
		resourceInitialHealthFilter: restored?.resourceInitialHealthFilter ?? "all",
		resourceNamespaceOverride: restored?.resourceNamespaceOverride ?? null,
		resourceBrowserPathState: restored?.resources ?? null,
		initialIncidentFilter: restored?.surfaces?.incidentFilter ?? "all",
	};
	}
export function workspacePathForWorkspace(
	workspace: SavedWorkspace,
	snapshot: PathStateWorkspaceSnapshot | null,
): PathStateWorkspaceSnapshot | null {
	return snapshot?.workspaceId === workspace.id ? snapshot : null;
}

function clearHandoffs(state: WorkspaceNavigationState): WorkspaceNavigationState {
	return {
		...state,
		targetHelmRelease: null,
		targetGitOpsApplication: null,
		restoreTargetResource: null,
		focusedResource: null,
		resourceInitialSearch: "",
		resourceInitialGitOpsFilter: "",
		resourceInitialHealthFilter: "all",
		resourceNamespaceOverride: null,
	};
}

export function navigateWorkspace(
	state: WorkspaceNavigationState,
	intent: WorkspaceNavigationIntent,
): WorkspaceNavigationState {
	if (intent.type === "clearHelmTarget") return { ...state, targetHelmRelease: null };
	if (intent.type === "clearGitOpsTarget") {
		return { ...state, targetGitOpsApplication: null };
	}
	if (intent.type === "focusResource") {
		return { ...state, focusedResource: intent.resource, restoreTargetResource: null };
	}
	if (intent.type === "updateResourceBrowserPath") {
		if (JSON.stringify(state.resourceBrowserPathState) === JSON.stringify(intent.pathState)) {
			return state;
		}
		return { ...state, resourceBrowserPathState: intent.pathState };
	}
	if (intent.type === "inspectResource") {
		return {
			...state,
			targetHelmRelease: null,
			targetGitOpsApplication: null,
			restoreTargetResource: null,
			focusedResource: intent.resource,
		};
	}
	if (intent.type === "clearResource") {
		return { ...state, focusedResource: null, restoreTargetResource: null };
	}
	if (intent.type === "closeSettings") {
		return { ...state, selectedNode: DEFAULT_WORKSPACE_NODE, viewMode: "overview" };
	}

	const cleared = clearHandoffs(state);
	if (intent.type === "openLauncher") return cleared;
	if (intent.type === "changeCluster") {
		return {
			...cleared,
			resourceGitOpsFocusApplication: null,
			selectedNode: null,
			viewMode: "resources",
		};
	}
	if (intent.type === "openSettings") {
		return { ...cleared, selectedNode: null, viewMode: "settings" };
	}
	if (intent.type === "openResources") {
		const namespaces = intent.namespaces;
		return {
			...cleared,
			resourceGitOpsFocusApplication: intent.gitOpsFocusApplication ?? null,
			resourceInitialSearch: intent.search ?? "",
			resourceInitialGitOpsFilter: intent.gitOpsFilter ?? "",
			resourceInitialHealthFilter: intent.healthFilter ?? "all",
			resourceBrowserPathState: null,
			resourceNamespaceOverride: Array.isArray(namespaces) ? namespaces : null,
			selectedNode:
				typeof namespaces === "string"
					? { type: "namespace", section: "namespaces", namespace: namespaces }
					: null,
			viewMode: "resources",
		};
	}
	if (intent.type === "openHelmRelease") {
		return {
			...cleared,
			targetHelmRelease: { name: intent.name, namespace: intent.namespace },
			selectedNode: { type: "section", section: "helm" },
			viewMode: "helm",
		};
	}
	if (intent.type === "openArgo") {
		return {
			...cleared,
			targetGitOpsApplication: intent.application ?? null,
			selectedNode: { type: "section", section: "argo" },
			viewMode: "argo",
		};
	}
	if (intent.type === "openIncidents") {
		return {
			...cleared,
			initialIncidentFilter: intent.filter ?? "all",
			selectedNode: { type: "section", section: "incidents" },
			viewMode: "incidents",
		};
	}
	if (intent.type === "openPortForwards") {
		return {
			...cleared,
			selectedNode: { type: "section", section: "portForwards" },
			viewMode: "portForwards",
		};
	}
	if (intent.type === "selectNode") {
		return {
			...cleared,
			resourceGitOpsFocusApplication: null,
			initialIncidentFilter:
				intent.node.type === "section" && intent.node.section === "incidents"
					? "all"
					: state.initialIncidentFilter,
			selectedNode: intent.node,
			viewMode: viewModeForTreeNode(intent.node),
		};
	}
	return {
		...cleared,
		resourceGitOpsFocusApplication: null,
		focusedResource: intent.resource,
		selectedNode: intent.node,
		viewMode: "resources",
	};
}

export function workspaceNavigationSnapshot(
	state: WorkspaceNavigationState,
	path: {
		workspaceId: string;
		expandedSections: string[];
		detail: PathStateResourceDetailState | null;
		surfaces: PathStateSurfacesState | null;
	},
): PathStateWorkspaceSnapshot {
	const focusedResource = state.focusedResource
		? resourceRefFromSummary(state.focusedResource)
		: null;
	const restoreTargetResource = focusedResource ??
		(state.restoreTargetResource
			? resourceRefFromSummary(state.restoreTargetResource)
			: null);
	return {
		workspaceId: path.workspaceId,
		selectedNode: state.selectedNode,
		expandedSections: path.expandedSections,
		viewMode: state.viewMode,
		resourceInitialSearch: state.resourceInitialSearch,
		resourceInitialGitOpsFilter: state.resourceInitialGitOpsFilter,
		resourceInitialHealthFilter: state.resourceInitialHealthFilter,
		resourceNamespaceOverride: state.resourceNamespaceOverride,
		focusedResource,
		restoreTargetResource,
		targetHelmRelease: state.targetHelmRelease,
		targetGitOpsApplication: state.targetGitOpsApplication,
		resources: state.resourceBrowserPathState,
		detail: path.detail,
		surfaces: path.surfaces,
	};
}

export function viewModeForTreeNode(nodeId: TreeNodeId | null): WorkspaceViewMode {
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
