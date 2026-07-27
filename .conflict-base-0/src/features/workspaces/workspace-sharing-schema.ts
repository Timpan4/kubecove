import type { SavedWorkspace, WorkspaceScope, WorkspaceShortcut } from "@/lib/workspace-model";

export const WORKSPACE_EXPORT_API_VERSION = "kubecove.dev/workspace/v1";

export type WorkspaceImportAction = "add" | "replace" | "skip";

interface SharedWorkspaceMetadata {
	name: string;
}

export interface SharedWorkspacePortForward {
	clusterContext: string;
	namespace: string;
	serviceName: string;
	servicePort: number;
	localPort?: number;
	label?: string;
}

interface SharedWorkspaceSpec {
	displayName: string;
	scope: WorkspaceScope;
	shortcuts: WorkspaceShortcut[];
	portForwards: SharedWorkspacePortForward[];
}

export interface SharedWorkspaceDocument {
	apiVersion: typeof WORKSPACE_EXPORT_API_VERSION;
	kind: "Workspace";
	metadata: SharedWorkspaceMetadata;
	spec: SharedWorkspaceSpec;
}

export interface SharedWorkspaceListDocument {
	apiVersion: typeof WORKSPACE_EXPORT_API_VERSION;
	kind: "WorkspaceList";
	metadata: SharedWorkspaceMetadata;
	items: SharedWorkspaceDocument[];
}

export interface WorkspaceImportItem {
	id: string;
	workspace: {
		sharedKey: string;
		name: string;
		scope: WorkspaceScope;
		shortcuts: WorkspaceShortcut[];
		portForwards: SharedWorkspacePortForward[];
	};
	existingWorkspaceId?: string;
	existingWorkspaceName?: string;
	defaultAction: WorkspaceImportAction;
}

export interface WorkspaceImportPreview {
	items: WorkspaceImportItem[];
}

export interface WorkspaceImportResult {
	workspaces: SavedWorkspace[];
	added: number;
	replaced: number;
	skipped: number;
}

export type WorkspaceImportDecisions = Record<string, WorkspaceImportAction>;
