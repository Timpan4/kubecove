import { queryKeys } from "@/lib/queryKeys";
import {
	listPodExecSessions,
	startPodExecSession,
	stopPodExecSession,
	type TauriClient,
} from "@/lib/tauri";
import type {
	PodExecSessionRequest,
	PodExecSessionSummary,
} from "@/lib/types";
import { workspaceScopeContexts, type SavedWorkspace } from "@/lib/workspace-model";
import {
	podExecSessionMatchesKubeconfigSource,
	sortPodExecSessions,
} from "./podExecHelpers";

export interface PodExecQuerySettings {
	enabled?: boolean;
	refetchInterval?: number | false;
}

export type InvalidatePodExecQueries = (options: {
	queryKey: readonly unknown[];
}) => Promise<unknown>;

export function podExecQueryOptions(
	client: TauriClient,
	settings: PodExecQuerySettings = {},
) {
	return {
		queryKey: queryKeys.podExecSessions(),
		queryFn: async () => sortPodExecSessions(await listPodExecSessions(client)),
		enabled: settings.enabled ?? true,
		placeholderData: (previousData: PodExecSessionSummary[] | undefined) => previousData,
		refetchInterval: settings.refetchInterval ?? 3_000,
	};
}

export function podExecSessionsForWorkspace(
	sessions: PodExecSessionSummary[],
	workspace: SavedWorkspace,
	kubeconfigSource?: string,
): PodExecSessionSummary[] {
	const contexts = workspaceScopeContexts(workspace.scope);
	return sessions.filter(
		(session) =>
			contexts.includes(session.clusterContext) &&
			podExecSessionMatchesKubeconfigSource(session, kubeconfigSource),
	);
}

export async function invalidatePodExecQueries(
	invalidateQueries: InvalidatePodExecQueries,
): Promise<void> {
	await invalidateQueries({ queryKey: queryKeys.podExecSessions() }).catch(() => undefined);
}

export async function startPodExec({
	client,
	request,
	channel,
	invalidateQueries,
}: {
	client: TauriClient;
	request: PodExecSessionRequest;
	channel: Parameters<typeof startPodExecSession>[2];
	invalidateQueries: InvalidatePodExecQueries;
}): Promise<PodExecSessionSummary> {
	const session = await startPodExecSession(client, request, channel);
	await invalidatePodExecQueries(invalidateQueries);
	return session;
}

export async function stopPodExec({
	client,
	sessionId,
	invalidateQueries,
}: {
	client: TauriClient;
	sessionId: string;
	invalidateQueries: InvalidatePodExecQueries;
}): Promise<boolean> {
	const stopped = await stopPodExecSession(client, sessionId);
	await invalidatePodExecQueries(invalidateQueries);
	return stopped;
}
