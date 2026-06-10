import type { MutableRefObject } from "react";
import type { DashboardViewMode } from "../lib/hooks";
import { resolveTreeScope, type TreeNodeId } from "../lib/tree-nav";
import { diagnosticLog } from "../lib/diagnostics";
import type { ResourceKindSelection } from "../lib/types";
import type { HealthFilter } from "../features/resources/helpers";
import type { SavedWorkspace } from "../lib/workspaces";

interface AppNavigationDeps {
	activeWorkspace: SavedWorkspace | null;
	viewMode: DashboardViewMode;
	settingsReturnViewModeRef: MutableRefObject<DashboardViewMode>;
	setActiveWorkspace: (id: string | null) => void;
	setSelectedNamespaces: (namespaces: string[]) => void;
	setSelectedKinds: (kinds: ResourceKindSelection[]) => void;
	setSelectedArgoAppFilter: (filter: string) => void;
	setSelectedTreeNode: (node: TreeNodeId | null) => void;
	setSelectedArgoApp: (app: null) => void;
	setSelectedFluxResource: (resource: null) => void;
	setSelectedResource: (resource: null) => void;
	setViewMode: (mode: DashboardViewMode) => void;
	setSelectedHelmRelease: (release: null) => void;
	setResourceInitialSearch: (search: string) => void;
	setResourceHealthFilter: (filter: HealthFilter) => void;
	setDismissedPortForwardRestoreWorkspaceId: (id: string | null) => void;
}

export interface AppNavigation {
	handleOpenResources: (namespace?: string, healthFilter?: HealthFilter) => void;
	handleOpenArgo: (argoApp?: string) => void;
	handleOpenIncidents: () => void;
	handleOpenLauncher: () => void;
	handleOpenSettings: () => void;
	handleBackFromSettings: () => void;
	handleOpenPortForwards: () => void;
	handleTreeNodeSelect: (nodeId: TreeNodeId) => void;
}

export function useAppNavigation(deps: AppNavigationDeps): AppNavigation {
	const {
		activeWorkspace,
		viewMode,
		settingsReturnViewModeRef,
		setActiveWorkspace,
		setSelectedNamespaces,
		setSelectedKinds,
		setSelectedArgoAppFilter,
		setSelectedTreeNode,
		setSelectedArgoApp,
		setSelectedFluxResource,
		setSelectedResource,
		setViewMode,
		setSelectedHelmRelease,
		setResourceInitialSearch,
		setResourceHealthFilter,
		setDismissedPortForwardRestoreWorkspaceId,
	} = deps;

	const handleOpenResources = (
		namespace?: string,
		healthFilter: HealthFilter = "all",
	) => {
		const workspace = activeWorkspace;
		if (!workspace) return;
		setSelectedNamespaces(
			namespace ? [namespace] : workspace.scope.namespaces,
		);
		setSelectedKinds(workspace.scope.kinds);
		setSelectedArgoAppFilter(
			workspace.scope.gitOpsFilter ?? workspace.scope.argoAppFilter,
		);
		setSelectedTreeNode(null);
		setSelectedArgoApp(null);
		setSelectedFluxResource(null);
		setSelectedHelmRelease(null);
		setSelectedResource(null);
		setResourceInitialSearch("");
		setResourceHealthFilter(healthFilter);
		setViewMode("resources");
	};

	const handleOpenArgo = (argoApp?: string) => {
		setSelectedArgoAppFilter(argoApp ?? "");
		setSelectedTreeNode({
			type: "kind",
			section: "argo",
			namespace: undefined,
			group: undefined,
			kind: "Applications",
		});
		setSelectedResource(null);
		setSelectedArgoApp(null);
		setSelectedFluxResource(null);
		setSelectedHelmRelease(null);
		setResourceInitialSearch("");
		setResourceHealthFilter("all");
		setViewMode("argo");
	};

	const handleOpenIncidents = () => {
		setViewMode("incidents");
		setSelectedTreeNode({ type: "section", section: "incidents" });
		setSelectedArgoApp(null);
		setSelectedFluxResource(null);
		setSelectedHelmRelease(null);
		setSelectedResource(null);
		setResourceInitialSearch("");
		setResourceHealthFilter("all");
	};

	const handleOpenLauncher = () => {
		setActiveWorkspace(null);
		setSelectedResource(null);
		setSelectedArgoApp(null);
		setSelectedFluxResource(null);
		setSelectedHelmRelease(null);
		setResourceInitialSearch("");
		setSelectedTreeNode(null);
		setResourceHealthFilter("all");
		setViewMode("resources");
	};

	const handleOpenSettings = () => {
		if (viewMode !== "settings") {
			settingsReturnViewModeRef.current = viewMode;
		}
		setViewMode("settings");
	};

	const handleBackFromSettings = () => {
		setViewMode(
			activeWorkspace ? settingsReturnViewModeRef.current : "resources",
		);
	};

	const handleOpenPortForwards = () => {
		setViewMode("portForwards");
		setSelectedTreeNode({ type: "section", section: "portForwards" });
		setSelectedResource(null);
		setSelectedArgoApp(null);
		setSelectedFluxResource(null);
		setSelectedHelmRelease(null);
		setResourceInitialSearch("");
		setResourceHealthFilter("all");
		if (activeWorkspace) {
			setDismissedPortForwardRestoreWorkspaceId(activeWorkspace.id);
		}
	};

	const handleTreeNodeSelect = (rawNodeId: TreeNodeId) => {
		// The GitOps section header lands on Argo CD Applications directly (mirroring
		// Helm → Releases) instead of a "select a resource type" stub.
		const nodeId: TreeNodeId =
			rawNodeId.type === "section" && rawNodeId.section === "argo"
				? {
						type: "kind",
						section: "argo",
						namespace: undefined,
						group: undefined,
						kind: "Applications",
					}
				: rawNodeId;
		const scope = resolveTreeScope(nodeId);
		diagnosticLog("app.tree.select", {
			type: nodeId.type,
			section: nodeId.section ?? "",
			namespace: nodeId.namespace ?? "",
			kind: nodeId.kind ?? "",
			argoMode: scope.argoMode,
			helmMode: scope.helmMode,
			incidentMode: scope.incidentMode,
			portForwardMode: scope.portForwardMode,
			rbacMode: scope.rbacMode,
		});

		// Tool sections own their inspector state.
		setSelectedArgoAppFilter("");

		if (nodeId.type === "section" && nodeId.section === "workspaceOverview") {
			setViewMode("overview");
			setSelectedArgoApp(null);
			setSelectedFluxResource(null);
			setSelectedHelmRelease(null);
			setSelectedResource(null);
			setResourceInitialSearch("");
			setResourceHealthFilter("all");
		} else if (scope.argoMode) {
			setViewMode("argo");
			setSelectedArgoApp(null);
			setSelectedFluxResource(null);
			setSelectedHelmRelease(null);
			setSelectedResource(null);
			setResourceInitialSearch("");
			setResourceHealthFilter("all");
		} else if (scope.helmMode) {
			setViewMode("helm");
			setSelectedArgoApp(null);
			setSelectedFluxResource(null);
			setSelectedHelmRelease(null);
			setSelectedResource(null);
			setResourceInitialSearch("");
			setResourceHealthFilter("all");
		} else if (scope.incidentMode) {
			setViewMode("incidents");
			setSelectedArgoApp(null);
			setSelectedFluxResource(null);
			setSelectedHelmRelease(null);
			setSelectedResource(null);
			setResourceInitialSearch("");
			setResourceHealthFilter("all");
		} else if (scope.portForwardMode) {
			setViewMode("portForwards");
			setSelectedArgoApp(null);
			setSelectedFluxResource(null);
			setSelectedHelmRelease(null);
			setSelectedResource(null);
			setResourceInitialSearch("");
			setResourceHealthFilter("all");
			if (activeWorkspace) {
				setDismissedPortForwardRestoreWorkspaceId(activeWorkspace.id);
			}
		} else if (scope.rbacMode) {
			setViewMode("rbac");
			setSelectedArgoApp(null);
			setSelectedFluxResource(null);
			setSelectedHelmRelease(null);
			setSelectedResource(null);
			setResourceInitialSearch("");
			setResourceHealthFilter("all");
		} else if (
			viewMode === "argo" ||
			viewMode === "helm" ||
			viewMode === "incidents" ||
			viewMode === "portForwards" ||
			viewMode === "rbac" ||
			viewMode === "settings" ||
			viewMode === "overview"
		) {
			// Leaving non-resource views clears inspector state.
			setViewMode("resources");
			setSelectedArgoApp(null);
			setSelectedFluxResource(null);
			setSelectedHelmRelease(null);
			setResourceInitialSearch("");
			setResourceHealthFilter("all");
		}
		if (
			!scope.argoMode &&
			!scope.helmMode &&
			!scope.incidentMode &&
			!scope.portForwardMode &&
			!scope.rbacMode
		) {
			setResourceInitialSearch("");
		}

		setSelectedTreeNode(nodeId);
	};

	return {
		handleOpenResources,
		handleOpenArgo,
		handleOpenIncidents,
		handleOpenLauncher,
		handleOpenSettings,
		handleBackFromSettings,
		handleOpenPortForwards,
		handleTreeNodeSelect,
	};
}
