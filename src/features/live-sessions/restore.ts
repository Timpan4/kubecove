import type { SavedWorkspace } from "@/lib/workspaces";

export function savedPortForwardCount(workspace: SavedWorkspace | null): number {
	return workspace?.portForwards?.length ?? 0;
}

export function shouldShowSavedPortForwardRestorePrompt({
	workspace,
	autoStart,
	dismissedWorkspaceId,
}: {
	workspace: SavedWorkspace | null;
	autoStart: boolean;
	dismissedWorkspaceId: string | null;
}): boolean {
	return (
		savedPortForwardCount(workspace) > 0 &&
		!autoStart &&
		workspace?.id !== dismissedWorkspaceId
	);
}

export function shouldAutoStartSavedPortForwards({
	workspace,
	autoStart,
	startedWorkspaceIds,
}: {
	workspace: SavedWorkspace | null;
	autoStart: boolean;
	startedWorkspaceIds: ReadonlySet<string>;
}): boolean {
	if (!workspace) return false;
	return (
		workspace.portForwards?.length > 0 &&
		autoStart &&
		!startedWorkspaceIds.has(workspace.id)
	);
}
