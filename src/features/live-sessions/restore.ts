import type { SavedWorkspace } from "@/lib/workspaces";

interface SavedPortForwardRestoreResult {
	ok: boolean;
}

export function savedPortForwardCount(workspace: SavedWorkspace | null): number {
	return workspace?.portForwards?.length ?? 0;
}

export function savedPortForwardStartFailureMessage(
	results: SavedPortForwardRestoreResult[],
): string | null {
	const failedCount = results.filter((result) => !result.ok).length;
	if (failedCount === 0) return null;
	return `${failedCount} saved ${failedCount === 1 ? "forward" : "forwards"} failed to start. Review port forwards for details.`;
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
