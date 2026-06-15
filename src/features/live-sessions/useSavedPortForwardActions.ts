import { useCallback, useMemo, useState, type Dispatch, type SetStateAction } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
	createTauriClient,
	listPortForwards,
	startPortForward,
	stopPodPortForward,
	type TauriClient,
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
	portForwardErrorMessage,
	isReusablePortForwardSession,
	savedPortForwardLocalPortConflict,
	savedPortForwardMatchesSession,
	savedPortForwardToRequest,
} from "./helpers";

export interface SavedPortForwardStartResult {
	portForwardId: string;
	ok: boolean;
	session?: PortForwardSessionSummary;
	error?: string;
	skipped?: boolean;
	conflict?: boolean;
}

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

interface StartSavedPortForwardOptions {
	client: TauriClient;
	workspaceId: string;
	portForward: SavedPortForward;
	knownSessions: PortForwardSessionSummary[];
	kubeconfigEnvVar?: string;
	updateSavedPortForward: ReturnType<
		typeof useWorkspaceStore.getState
	>["updateSavedPortForward"];
}

async function startSavedPortForward({
	client,
	workspaceId,
	portForward,
	knownSessions,
	kubeconfigEnvVar,
	updateSavedPortForward,
}: StartSavedPortForwardOptions): Promise<SavedPortForwardStartResult> {
	const matchingSessions = knownSessions.filter((session) =>
		savedPortForwardMatchesSession(portForward, session, kubeconfigEnvVar),
	);
	const existingSession = matchingSessions.find(isReusablePortForwardSession);
	if (existingSession) {
		updateSavedPortForward(workspaceId, portForward.id, {
			lastStartedAt: existingSession.startedAt,
			lastStatus: existingSession.status,
			lastError: undefined,
		});
		return {
			portForwardId: portForward.id,
			ok: true,
			session: existingSession,
			skipped: true,
		};
	}

	const conflictingSession = savedPortForwardLocalPortConflict(
		portForward,
		knownSessions,
	);
	if (conflictingSession) {
		const message = `Local port ${conflictingSession.localPort} is already used by ${conflictingSession.namespace}/${conflictingSession.targetKind}/${conflictingSession.targetName}.`;
		updateSavedPortForward(workspaceId, portForward.id, {
			lastStatus: "error",
			lastError: message,
		});
		return {
			portForwardId: portForward.id,
			ok: false,
			error: message,
			skipped: true,
			conflict: true,
		};
	}

	updateSavedPortForward(workspaceId, portForward.id, {
		lastStatus: "starting",
		lastError: undefined,
	});
	try {
		for (const session of matchingSessions) {
			await stopPodPortForward(client, session.id);
		}
		const session = await startPortForward(
			client,
			savedPortForwardToRequest(portForward, kubeconfigEnvVar),
		);
		updateSavedPortForward(workspaceId, portForward.id, {
			lastStartedAt: session.startedAt,
			lastStatus: session.status,
			lastError: undefined,
		});
		return { portForwardId: portForward.id, ok: true, session };
	} catch (error) {
		const message = portForwardErrorMessage(error);
		updateSavedPortForward(workspaceId, portForward.id, {
			lastStatus: "error",
			lastError: message,
		});
		return { portForwardId: portForward.id, ok: false, error: message };
	}
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
					kubeconfigEnvVar,
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
				let knownSessions =
					activeSessions !== undefined
						? activeSessions
						: await listPortForwards(client).catch(() => []);
				const results: SavedPortForwardStartResult[] = [];
				for (const portForward of portForwards) {
					const result = await startOne(portForward, knownSessions);
					results.push(result);
					if (result.session) {
						knownSessions = [...knownSessions, result.session];
					}
				}
				return results;
			})().catch((error) => {
				setStartingAll(false);
				throw error;
			});
			setStartingAll(false);
			return results;
		},
		[activeSessions, client, startOne, workspace],
	);

	return {
		startOne,
		startAll,
		startingIds,
		startingAll,
	};
}
