import type { ClusterContext } from "@/lib/types";
import {
	type CreateWorkspaceInput,
	DEFAULT_WORKSPACE_KINDS,
	type SavedWorkspace,
} from "@/lib/workspace-model";

type NamespaceDiscoveryStatus = "loading" | "failed" | "ready";

export function getWorkspaceCreationAvailability(
	effectiveContext: string,
	selectedContextMissing: boolean,
	namespaceDiscovery: NamespaceDiscoveryStatus,
): { canCreate: boolean; disabledReason: string | null } {
	if (!effectiveContext) {
		return {
			canCreate: false,
			disabledReason: "Select a context before creating this workspace.",
		};
	}
	if (selectedContextMissing) {
		return {
			canCreate: false,
			disabledReason: "Select an available context before creating this workspace.",
		};
	}
	if (namespaceDiscovery === "failed") {
		return {
			canCreate: false,
			disabledReason: "Retry namespace discovery before creating this workspace.",
		};
	}
	if (namespaceDiscovery === "loading") {
		return {
			canCreate: false,
			disabledReason: "Wait for namespace discovery to finish.",
		};
	}
	return { canCreate: true, disabledReason: null };
}

export function pickEffectiveContext(
	selectedContext: string,
	availableContexts: ClusterContext[],
): string {
	return (
		selectedContext ||
		availableContexts.find((context) => context.isCurrent)?.name ||
		availableContexts[0]?.name ||
		""
	);
}

export function uniqueWorkspaceContexts(
	primaryContext: string,
	groupContexts: string[],
): string[] {
	return Array.from(new Set([primaryContext, ...groupContexts].filter(Boolean)));
}

export function buildWorkspaceInput({
	name,
	effectiveContext,
	selectedClusterContexts,
	selectedNamespaces,
	editingWorkspace,
}: {
	name: string;
	effectiveContext: string;
	selectedClusterContexts: string[];
	selectedNamespaces: string[];
	editingWorkspace?: SavedWorkspace | null;
}): CreateWorkspaceInput {
	const trimmedName = name.trim();
	const workspaceName = trimmedName || effectiveContext;
	const clusterGroupName =
		selectedClusterContexts.length > 1 ? `${workspaceName} group` : undefined;
	return {
		name: workspaceName,
		clusterContext: effectiveContext,
		clusterContexts: selectedClusterContexts,
		clusterGroupName,
		namespaces: selectedNamespaces,
		kinds: editingWorkspace?.scope.kinds ?? DEFAULT_WORKSPACE_KINDS,
		shortcutPreferences: editingWorkspace?.scope.shortcutPreferences,
	};
}
