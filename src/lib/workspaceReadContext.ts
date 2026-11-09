import type { KubeconfigSourcesSummary } from "@/lib/types";
import { type SavedWorkspace, workspaceScopeContexts } from "@/lib/workspace-model";

export interface WorkspaceReadContext {
	workspaceId: string;
	/** The primary context, which primary-only pages read. */
	clusterContext: string;
	/** Every workspace context in rank order: the primary first, then group members. */
	clusterContexts: readonly string[];
	namespaceScope: readonly string[];
	kubeconfigSourceKey?: string;
	sourceReady: boolean;
	sourceError: unknown;
	showKubeconfigSourceLabels: boolean;
}

export function buildWorkspaceReadContext({
	workspace,
	sources,
	sourceSucceeded,
	sourceFailed,
	sourceError,
}: {
	workspace: SavedWorkspace;
	sources?: KubeconfigSourcesSummary;
	sourceSucceeded: boolean;
	sourceFailed: boolean;
	sourceError: unknown;
}): WorkspaceReadContext {
	return {
		workspaceId: workspace.id,
		clusterContext: workspace.scope.clusterContext,
		clusterContexts: workspaceScopeContexts(workspace.scope),
		namespaceScope: [...workspace.scope.namespaces],
		kubeconfigSourceKey: sources?.sourceKey,
		sourceReady: sourceSucceeded || sourceFailed,
		sourceError: sourceFailed ? sourceError : null,
		showKubeconfigSourceLabels: sources?.showSourceLabels ?? false,
	};
}
