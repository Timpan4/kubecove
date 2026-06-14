import type { MutableRefObject } from "react";
import { ARGO_NAV_KINDS } from "@/features/gitops/gitops-nav";
import type { DashboardViewMode, OpenViewOptions } from "../lib/hooks";
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
	setViewMode: (mode: DashboardViewMode) => void;
	openView: (mode: DashboardViewMode, options?: OpenViewOptions) => void;
	setResourceInitialSearch: (search: string) => void;
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
		setViewMode,
		openView,
		setResourceInitialSearch,
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
		openView("resources", {
			treeNode: null,
			argoAppFilter:
				workspace.scope.gitOpsFilter ?? workspace.scope.argoAppFilter,
			healthFilter,
		});
	};

	const handleOpenArgo = (argoApp?: string) => {
		openView("argo", {
			argoAppFilter: argoApp ?? "",
			treeNode: {
				type: "kind",
				section: "argo",
				namespace: undefined,
				group: undefined,
				kind: ARGO_NAV_KINDS[0].label,
			},
		});
	};

	const handleOpenIncidents = () => {
		openView("incidents", {
			treeNode: { type: "section", section: "incidents" },
		});
	};

	const handleOpenLauncher = () => {
		setActiveWorkspace(null);
		openView("resources", { treeNode: null });
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
		openView("portForwards", {
			treeNode: { type: "section", section: "portForwards" },
		});
		if (activeWorkspace) {
			setDismissedPortForwardRestoreWorkspaceId(activeWorkspace.id);
		}
	};

	const handleTreeNodeSelect = (rawNodeId: TreeNodeId) => {
		const nodeId: TreeNodeId = rawNodeId;
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

		if (nodeId.type === "section" && nodeId.section === "workspaceOverview") {
			openView("overview", { treeNode: nodeId });
		} else if (scope.argoMode) {
			openView("argo", { treeNode: nodeId });
		} else if (scope.helmMode) {
			openView("helm", { treeNode: nodeId });
		} else if (scope.incidentMode) {
			openView("incidents", { treeNode: nodeId });
		} else if (scope.portForwardMode) {
			openView("portForwards", { treeNode: nodeId });
			if (activeWorkspace) {
				setDismissedPortForwardRestoreWorkspaceId(activeWorkspace.id);
			}
		} else if (scope.rbacMode) {
			openView("rbac", { treeNode: nodeId });
		} else if (
			viewMode === "argo" ||
			viewMode === "helm" ||
			viewMode === "incidents" ||
			viewMode === "portForwards" ||
			viewMode === "rbac" ||
			viewMode === "settings" ||
			viewMode === "overview"
		) {
			openView("resources", { treeNode: nodeId });
		} else {
			setSelectedArgoAppFilter("");
			setResourceInitialSearch("");
			setSelectedTreeNode(nodeId);
		}
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
