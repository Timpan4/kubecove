import { useCallback, useMemo, useState, type Dispatch, type SetStateAction } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
	createTauriClient,
	listPortForwards,
	startPortForward,
	type TauriClient,
} from "@/lib/tauri";
import { queryKeys } from "@/lib/queryKeys";
import type { PortForwardSessionSummary } from "@/lib/types";
import {
	useWorkspaceStore,
	type SavedPortForward,
	type SavedWorkspace,
} from "@/lib/workspaces";
import {
	isReusablePortForwardSession,
	savedPortForwardMatchesSession,
	savedPortForwardToRequest,
} from "./helpers";

export interface SavedPortForwardStartResult {
	portForwardId: string;
	ok: boolean;
	session?: PortForwardSessionSummary;
	error?: string;
	skipped?: boolean;
}

function getErrorMessage(error: unknown): string {
	if (error instanceof Error) return error.message;
	if (typeof error === "string") return error;
	if (
		typeof error === "object" &&
		error !== null &&
		"message" in error &&
		typeof error.message === "string"
	) {
		return error.message;
	}
	return "Unknown error";
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
	updateSavedPortForward: ReturnType<
		typeof useWorkspaceStore.getState
	>["updateSavedPortForward"];
}

async function startSavedPortForward({
	client,
	workspaceId,
	portForward,
	knownSessions,
	updateSavedPortForward,
}: StartSavedPortForwardOptions): Promise<SavedPortForwardStartResult> {
	const existingSession = knownSessions.find(
		(session) =>
			isReusablePortForwardSession(session) &&
			savedPortForwardMatchesSession(portForward, session),
	);
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

	updateSavedPortForward(workspaceId, portForward.id, {
		lastStatus: "starting",
		lastError: undefined,
	});
	try {
		const session = await startPortForward(
			client,
			savedPortForwardToRequest(portForward),
		);
		updateSavedPortForward(workspaceId, portForward.id, {
			lastStartedAt: session.startedAt,
			lastStatus: session.status,
			lastError: undefined,
		});
		return { portForwardId: portForward.id, ok: true, session };
	} catch (error) {
		const message = getErrorMessage(error);
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
	const queryClient = useQueryClient();
	const updateSavedPortForward = useWorkspaceStore(
		(state) => state.updateSavedPortForward,
	);
	const [startingIds, setStartingIds] = useState<Set<string>>(new Set());
	const [startingAll, setStartingAll] = useState(false);

	const startOne = useCallback(
		async (
			portForward: SavedPortForward,
			knownSessions: PortForwardSessionSummary[] = activeSessions ?? [],
		): Promise<SavedPortForwardStartResult> => {
			if (!workspace) {
				return {
					portForwardId: portForward.id,
					ok: false,
					error: "No active workspace",
				};
			}
			setStartingId(setStartingIds, portForward.id, true);
			try {
				const result = await startSavedPortForward({
					client,
					workspaceId: workspace.id,
					portForward,
					knownSessions,
					updateSavedPortForward,
				});
				await queryClient.invalidateQueries({
					queryKey: queryKeys.portForwards(),
				});
				return result;
			} finally {
				setStartingId(setStartingIds, portForward.id, false);
			}
		},
		[activeSessions, client, queryClient, updateSavedPortForward, workspace],
	);

	const startAll = useCallback(
		async (
			portForwards: SavedPortForward[] = workspace?.portForwards ?? [],
		): Promise<SavedPortForwardStartResult[]> => {
			if (!workspace || portForwards.length === 0) return [];
			setStartingAll(true);
			try {
				let knownSessions =
					activeSessions ?? (await listPortForwards(client).catch(() => []));
				const results: SavedPortForwardStartResult[] = [];
				for (const portForward of portForwards) {
					const result = await startOne(portForward, knownSessions);
					results.push(result);
					if (result.session) {
						knownSessions = [...knownSessions, result.session];
					}
				}
				return results;
			} finally {
				setStartingAll(false);
			}
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
