import type { CSSProperties } from "react";
import type { DashboardViewMode } from "@/lib/hooks";
import type { ResourceKindSelection } from "@/lib/types";
import {
	discoveredResourceKindKey,
	type TreeNodeId,
	type TreeScope,
} from "@/lib/tree-nav";
import type { SavedWorkspace } from "@/lib/workspaces";

export const SECTION_LABELS: Record<string, string> = {
	workspaceOverview: "Workspace Overview",
	clusterOverview: "Cluster Overview",
	namespaces: "Namespaces",
	workloads: "Workloads",
	network: "Network",
	config: "Config",
	storage: "Storage",
	discovered: "Discovered",
	argo: "GitOps",
	helm: "Helm",
	incidents: "Incidents",
	portForwards: "Port Forwards",
	rbac: "RBAC",
};

export const SIDEBAR_PROVIDER_STYLE = {
	"--sidebar-width": "260px",
} as CSSProperties;

export function resourceKindLabel(kind: ResourceKindSelection): string {
	return typeof kind === "string" ? kind : kind.kind;
}

export function resourceKindLogKey(kind: ResourceKindSelection): string {
	return typeof kind === "string" ? kind : discoveredResourceKindKey(kind);
}

export function hasDiscoveredKind(kinds: ResourceKindSelection[]): boolean {
	return kinds.some((kind) => typeof kind !== "string");
}

export function canQueryResourceScope({
	clusterContext,
	kinds,
	namespaces,
	scope,
	hasActiveWorkspace,
}: {
	clusterContext: string;
	kinds: ResourceKindSelection[];
	namespaces: string[];
	scope: TreeScope;
	hasActiveWorkspace: boolean;
}): boolean {
	if (!clusterContext || kinds.length === 0) return false;
	if (scope.clusterScoped || scope.namespace || namespaces.length > 0) return true;
	if (scope.kinds.length > 0) return true;
	if (hasActiveWorkspace && scope.section === null) return true;
	return hasDiscoveredKind(kinds);
}

export function hasAppDetailPanel(
	viewMode: DashboardViewMode,
	hasSelectedHelmRelease: boolean,
	hasSelectedArgoApp: boolean,
	hasSelectedResource: boolean,
): boolean {
	if (viewMode === "helm") return hasSelectedHelmRelease;
	if (viewMode === "argo") return hasSelectedArgoApp;
	return hasSelectedResource;
}

export function getAppContentTitle({
	activeWorkspace,
	scope,
	selectedTreeNode,
	viewMode,
}: {
	activeWorkspace: SavedWorkspace | null;
	scope: TreeScope;
	selectedTreeNode: TreeNodeId | null;
	viewMode: DashboardViewMode;
}): string {
	if (viewMode === "overview") return activeWorkspace?.name ?? "Workspace";
	if (viewMode === "settings") return "Settings";
	if (viewMode === "helm") return "Helm Releases";
	if (viewMode === "incidents") return "Incident Cockpit";
	if (viewMode === "portForwards") return "Port Forwards";
	if (viewMode === "rbac") {
		if (selectedTreeNode?.type === "kind" && selectedTreeNode.kind) {
			return selectedTreeNode.kind;
		}
		return "RBAC";
	}
	if (viewMode === "argo") {
		if (selectedTreeNode?.type === "kind" && selectedTreeNode.kind) {
			return selectedTreeNode.kind;
		}
		return "GitOps";
	}
	if (!scope.section) return "Kubernetes Resources";
	if (scope.section === "clusterOverview") {
		if (scope.kinds.length === 1)
			return `${resourceKindLabel(scope.kinds[0])} Resources`;
		if (scope.kinds.length > 1) return "Cluster Overview";
		return "Cluster Overview";
	}
	if (scope.section === "namespaces" && scope.namespace) {
		if (scope.group && scope.kinds.length > 0) {
			return `${scope.namespace} / ${scope.group}`;
		}
		return scope.namespace;
	}
	if (scope.group) return scope.group;
	if (scope.kinds.length === 1)
		return `${resourceKindLabel(scope.kinds[0])} Resources`;
	if (scope.kinds.length > 1)
		return SECTION_LABELS[scope.section] ?? scope.section;
	return SECTION_LABELS[scope.section] ?? scope.section;
}
