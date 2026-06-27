import {
	DEFAULT_WORKSPACE_KINDS,
	type CreateWorkspaceInput,
	type SavedWorkspace,
} from "@/lib/workspace-model";
import type { ClusterContext } from "@/lib/types";

export function pickEffectiveContext(
	selectedContext: string,
	contexts: ClusterContext[],
): string {
	return (
		selectedContext ||
		contexts.find((context) => context.isCurrent)?.name ||
		contexts[0]?.name ||
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
