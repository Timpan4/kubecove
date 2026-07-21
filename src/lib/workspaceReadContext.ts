import type { KubeconfigSourcesSummary } from "@/lib/types";
import type { SavedWorkspace } from "@/lib/workspace-model";

export interface WorkspaceReadContext {
	workspaceId: string;
	clusterContext: string;
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
		namespaceScope: [...workspace.scope.namespaces],
		kubeconfigSourceKey: sources?.sourceKey,
		sourceReady: sourceSucceeded || sourceFailed,
		sourceError: sourceFailed ? sourceError : null,
		showKubeconfigSourceLabels: sources?.showSourceLabels ?? false,
	};
}
