import { queryKeys } from "@/lib/queryKeys";
import type { TauriClient } from "@/lib/tauri";
import { listPortForwards, startPortForward, stopPodPortForward } from "@/lib/tauri";
import type { PortForwardSessionSummary } from "@/lib/types";
import type {
	SavedPortForward,
	SavedPortForwardUpdates,
	SavedWorkspace,
} from "@/lib/workspaces";
import {
	isReusablePortForwardSession,
	portForwardErrorMessage,
	portForwardSessionToRequest,
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

export type UpdateSavedPortForward = (
	workspaceId: string,
	portForwardId: string,
	updates: SavedPortForwardUpdates,
) => void;

interface StartSavedPortForwardOptions {
	client: TauriClient;
	workspaceId: string;
	portForward: SavedPortForward;
	knownSessions: PortForwardSessionSummary[];
	kubeconfigSource?: string;
	updateSavedPortForward: UpdateSavedPortForward;
}

export async function startSavedPortForward({
	client,
	workspaceId,
	portForward,
	knownSessions,
	kubeconfigSource,
	updateSavedPortForward,
}: StartSavedPortForwardOptions): Promise<SavedPortForwardStartResult> {
	const matchingSessions = knownSessions.filter((session) =>
		savedPortForwardMatchesSession(portForward, session, kubeconfigSource),
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
			savedPortForwardToRequest(portForward, kubeconfigSource),
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

export async function startSavedPortForwards({
	client,
	workspace,
	portForwards = workspace?.portForwards ?? [],
	activeSessions,
	kubeconfigSource,
	updateSavedPortForward,
}: {
	client: TauriClient;
	workspace: SavedWorkspace | null;
	portForwards?: SavedPortForward[];
	activeSessions?: PortForwardSessionSummary[];
	kubeconfigSource?: string;
	updateSavedPortForward: UpdateSavedPortForward;
}): Promise<SavedPortForwardStartResult[]> {
	if (!workspace || portForwards.length === 0) return [];
	let knownSessions =
		activeSessions !== undefined
			? activeSessions
			: await listPortForwards(client).catch(() => []);
	const results: SavedPortForwardStartResult[] = [];
	for (const portForward of portForwards) {
		const result = await startSavedPortForward({
			client,
			workspaceId: workspace.id,
			portForward,
			knownSessions,
			kubeconfigSource,
			updateSavedPortForward,
		});
		results.push(result);
		if (result.session) {
			knownSessions = [...knownSessions, result.session];
		}
	}
	return results;
}

export function invalidatePortForwardQueries(
	invalidateQueries: (options: { queryKey: readonly unknown[] }) => Promise<unknown>,
): Promise<unknown> {
	return invalidateQueries({ queryKey: queryKeys.portForwards() });
}

export async function reconnectPortForwardSession({
	client,
	session,
}: {
	client: TauriClient;
	session: PortForwardSessionSummary;
}): Promise<PortForwardSessionSummary> {
	await stopPodPortForward(client, session.id);
	return startPortForward(client, portForwardSessionToRequest(session));
}
