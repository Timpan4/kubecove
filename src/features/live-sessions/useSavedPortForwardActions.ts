import { useCallback, useMemo, useState, type Dispatch, type SetStateAction } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
	createTauriClient,
	listPortForwards,
} from "@/lib/tauri";
import { queryKeys } from "@/lib/queryKeys";
import { useSettingsState } from "@/lib/settings";
import type { PortForwardSessionSummary } from "@/lib/types";
import {
	useWorkspaceStore,
	type SavedPortForward,
	type SavedWorkspace,
} from "@/lib/workspaces";
import {
	startSavedPortForward,
	startSavedPortForwards,
	type SavedPortForwardStartResult,
} from "./saved-port-forward-actions";

function setStartingId(
	setter: Dispatch<SetStateAction<Set<string>>>,
	id: string,
	starting: boolean,
): void {
	setter((current) => {
		const next = new Set(current);
		if (starting) next.add(id);
		else next.delete(id);
		return next;
	});
}

export function useSavedPortForwardActions(
	workspace: SavedWorkspace | null,
	activeSessions?: PortForwardSessionSummary[],
) {
	const client = useMemo(() => createTauriClient(), []);
	const kubeconfigEnvVar = useSettingsState((state) => state.kubeconfigSourceKey);
	const queryClient = useQueryClient();
	const updateSavedPortForward = useWorkspaceStore(
		(state) => state.updateSavedPortForward,
	);
	const [startingIds, setStartingIds] = useState<Set<string>>(new Set());
	const [startingAll, setStartingAll] = useState(false);

	const startOne = useCallback(
		async (
			portForward: SavedPortForward,
			knownSessions?: PortForwardSessionSummary[],
		): Promise<SavedPortForwardStartResult> => {
			if (!workspace) {
				return {
					portForwardId: portForward.id,
					ok: false,
					error: "No active workspace",
				};
			}
			const currentSessions =
				knownSessions ?? activeSessions ?? (await listPortForwards(client).catch(() => []));
			setStartingId(setStartingIds, portForward.id, true);
			const result = await (async () => {
				const result = await startSavedPortForward({
					client,
					workspaceId: workspace.id,
					portForward,
					knownSessions: currentSessions,
					kubeconfigSource: kubeconfigEnvVar,
					updateSavedPortForward,
				});
				await queryClient.invalidateQueries({
					queryKey: queryKeys.portForwards(),
				});
				return result;
			})().catch((error) => {
				setStartingId(setStartingIds, portForward.id, false);
				throw error;
			});
			setStartingId(setStartingIds, portForward.id, false);
			return result;
		},
		[
			activeSessions,
			client,
			queryClient,
			kubeconfigEnvVar,
			updateSavedPortForward,
			workspace,
		],
	);

	const startAll = useCallback(
		async (
			portForwards: SavedPortForward[] = workspace?.portForwards ?? [],
		): Promise<SavedPortForwardStartResult[]> => {
			if (!workspace || portForwards.length === 0) return [];
			setStartingAll(true);
			const results = await (async () => {
				return startSavedPortForwards({
					client,
					workspace,
					portForwards,
					activeSessions,
					kubeconfigSource: kubeconfigEnvVar,
					updateSavedPortForward,
				});
			})().catch((error) => {
				setStartingAll(false);
				throw error;
			});
			setStartingAll(false);
			return results;
		},
		[activeSessions, client, kubeconfigEnvVar, updateSavedPortForward, workspace],
	);

	return {
		startOne,
		startAll,
		startingIds,
		startingAll,
	};
}
