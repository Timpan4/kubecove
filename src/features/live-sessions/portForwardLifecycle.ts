import { queryKeys } from "@/lib/queryKeys";
import type { TauriClient } from "@/lib/tauri";
import {
	listPortForwards,
	startPodPortForward,
	stopPodPortForward,
} from "@/lib/tauri";
import type { PortForwardSessionSummary, ResourceSummary } from "@/lib/types";
import type { SavedPortForward, SavedWorkspace } from "@/lib/workspace-model";
import { workspaceScopeContexts } from "@/lib/workspace-model";
import {
	portForwardSessionMatchesKubeconfigSource,
	sortPortForwardSessions,
} from "./helpers";
import {
	reconnectPortForwardSession,
	startSavedPortForward as startSavedPortForwardPrimitive,
	startSavedPortForwards as startSavedPortForwardsPrimitive,
	type SavedPortForwardStartResult,
	type UpdateSavedPortForward,
} from "./saved-port-forward-actions";

export interface PortForwardQuerySettings {
	enabled?: boolean;
	refetchInterval?: number | false;
}

export type InvalidatePortForwardQueries = (options: {
	queryKey: readonly unknown[];
}) => Promise<unknown>;

export function portForwardQueryOptions(
	client: TauriClient,
	settings: PortForwardQuerySettings = {},
) {
	return {
		queryKey: queryKeys.portForwards(),
		queryFn: async () => sortPortForwardSessions(await listPortForwards(client)),
		enabled: settings.enabled ?? true,
		placeholderData: (previousData: PortForwardSessionSummary[] | undefined) => previousData,
		refetchInterval: settings.refetchInterval ?? 3_000,
	};
}

export function portForwardSessionsForWorkspace(
	sessions: PortForwardSessionSummary[],
	workspace: SavedWorkspace,
	kubeconfigSource?: string,
): PortForwardSessionSummary[] {
	const contexts = workspaceScopeContexts(workspace.scope);
	return sessions.filter(
		(session) =>
			contexts.includes(session.clusterContext) &&
			portForwardSessionMatchesKubeconfigSource(session, kubeconfigSource),
	);
}

async function invalidate(invalidateQueries: InvalidatePortForwardQueries): Promise<void> {
	await invalidateQueries({ queryKey: queryKeys.portForwards() });
}

export async function stopPortForward({
	client,
	sessionId,
	invalidateQueries,
}: {
	client: TauriClient;
	sessionId: string;
	invalidateQueries: InvalidatePortForwardQueries;
}): Promise<void> {
	await stopPodPortForward(client, sessionId);
	await invalidate(invalidateQueries);
}

export async function reconnectPortForward({
	client,
	session,
	invalidateQueries,
}: {
	client: TauriClient;
	session: PortForwardSessionSummary;
	invalidateQueries: InvalidatePortForwardQueries;
}): Promise<PortForwardSessionSummary> {
	const reconnected = await reconnectPortForwardSession({ client, session });
	await invalidate(invalidateQueries);
	return reconnected;
}

export async function startResourcePortForward({
	client,
	resource,
	remotePort,
	localPort,
	kubeconfigSource,
	invalidateQueries,
}: {
	client: TauriClient;
	resource: ResourceSummary;
	remotePort: number;
	localPort?: number;
	kubeconfigSource?: string;
	invalidateQueries: InvalidatePortForwardQueries;
}): Promise<PortForwardSessionSummary> {
	if (!resource.namespace || (resource.kind !== "Pod" && resource.kind !== "Service")) {
		throw new Error("Pod or Service target with namespace is required");
	}
	const session = await startPodPortForward(client, {
		clusterContext: resource.cluster,
		kubeconfigEnvVar: kubeconfigSource?.startsWith("kubeconfigSource=")
			? undefined
			: kubeconfigSource,
		namespace: resource.namespace,
		targetKind: resource.kind,
		targetName: resource.name,
		remotePort,
		localPort,
	});
	await invalidate(invalidateQueries);
	return session;
}

export async function startSavedPortForward({
	invalidateQueries,
	knownSessions,
	...options
}: Omit<Parameters<typeof startSavedPortForwardPrimitive>[0], "knownSessions"> & {
	knownSessions?: PortForwardSessionSummary[];
	invalidateQueries: InvalidatePortForwardQueries;
}): Promise<SavedPortForwardStartResult> {
	const result = await startSavedPortForwardPrimitive({
		...options,
		knownSessions: knownSessions ?? (await listPortForwards(options.client).catch(() => [])),
	});
	await invalidate(invalidateQueries);
	return result;
}

export async function startSavedPortForwards({
	invalidateQueries,
	...options
}: Parameters<typeof startSavedPortForwardsPrimitive>[0] & {
	invalidateQueries: InvalidatePortForwardQueries;
}): Promise<SavedPortForwardStartResult[]> {
	const results = await startSavedPortForwardsPrimitive(options);
	await invalidate(invalidateQueries);
	return results;
}

export type { SavedPortForward, SavedPortForwardStartResult, UpdateSavedPortForward };
