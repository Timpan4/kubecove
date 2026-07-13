import { queryKeys } from "@/lib/queryKeys";
import type { TauriClient } from "@/lib/tauri";
import {
	listPortForwards,
	startPodPortForward,
	startPortForward,
	stopPodPortForward,
} from "@/lib/tauri";
import type { PortForwardSessionSummary, ResourceSummary } from "@/lib/types";
import type {
	SavePortForwardInput,
	SavedPortForward,
	SavedPortForwardUpdates,
	SavedWorkspace,
} from "@/lib/workspace-model";
import { workspaceScopeContexts } from "@/lib/workspace-model";
import {
	isReusablePortForwardSession,
	portForwardErrorMessage,
	portForwardRequestKubeconfigEnvVar,
	portForwardSessionMatchesKubeconfigSource,
	portForwardSessionToRequest,
	savedPortForwardLocalPortConflict,
	savedPortForwardMatchesSession,
	savedPortForwardToRequest,
	sortPortForwardSessions,
} from "./helpers";
import {
	parseSavedPortForwardForm,
	type SavedPortForwardFormValues,
} from "./portForwardForms";

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

export function parseSavedPortForwardForWorkspace(
	values: SavedPortForwardFormValues,
	workspace: SavedWorkspace,
): SavePortForwardInput | string {
	const parsed = parseSavedPortForwardForm(values);
	if (typeof parsed === "string") return parsed;
	if (!workspaceScopeContexts(workspace.scope).includes(parsed.clusterContext)) {
		return "Cluster context must be in the current workspace scope.";
	}
	return parsed;
}

export function savedPortForwardStartFailureMessage(
	results: Array<Pick<SavedPortForwardStartResult, "ok" | "conflict">>,
): string | null {
	const failedCount = results.filter((result) => !result.ok).length;
	if (failedCount === 0) return null;
	const conflictCount = results.filter((result) => result.conflict).length;
	if (conflictCount > 0) {
		return `${conflictCount} saved ${conflictCount === 1 ? "forward has" : "forwards have"} local port conflicts. Review port forwards for details.`;
	}
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
		(workspace?.portForwards?.length ?? 0) > 0 &&
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

export async function invalidatePortForwardQueries(
	invalidateQueries: InvalidatePortForwardQueries,
): Promise<void> {
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
	await invalidatePortForwardQueries(invalidateQueries);
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
	await stopPodPortForward(client, session.id);
	try {
		return await startPortForward(
			client,
			portForwardSessionToRequest(session),
		);
	} finally {
		await invalidatePortForwardQueries(invalidateQueries);
	}
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
		kubeconfigEnvVar: portForwardRequestKubeconfigEnvVar(kubeconfigSource),
		namespace: resource.namespace,
		targetKind: resource.kind,
		targetName: resource.name,
		remotePort,
		localPort,
	});
	await invalidatePortForwardQueries(invalidateQueries);
	return session;
}

interface StartSavedPortForwardOptions {
	client: TauriClient;
	workspaceId: string;
	portForward: SavedPortForward;
	knownSessions: PortForwardSessionSummary[];
	kubeconfigSource?: string;
	updateSavedPortForward: UpdateSavedPortForward;
}

async function startSavedPortForwardCore({
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

	const matchingSessionIds = new Set(matchingSessions.map((session) => session.id));
	const conflictingSession = savedPortForwardLocalPortConflict(
		portForward,
		knownSessions.filter((session) => !matchingSessionIds.has(session.id)),
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
	let result: SavedPortForwardStartResult;
	try {
		for (const session of matchingSessions) {
			await stopPodPortForward(client, session.id);
		}
		const started = await startPortForward(
			client,
			savedPortForwardToRequest(portForward, kubeconfigSource),
		);
		updateSavedPortForward(workspaceId, portForward.id, {
			lastStartedAt: started.startedAt,
			lastStatus: started.status,
			lastError: undefined,
		});
		result = { portForwardId: portForward.id, ok: true, session: started };
	} catch (error) {
		const message = portForwardErrorMessage(error);
		updateSavedPortForward(workspaceId, portForward.id, {
			lastStatus: "error",
			lastError: message,
		});
		result = { portForwardId: portForward.id, ok: false, error: message };
	}
	return result;
}

export async function startSavedPortForward({
	client,
	workspaceId,
	portForward,
	knownSessions,
	kubeconfigSource,
	updateSavedPortForward,
	invalidateQueries,
}: Omit<StartSavedPortForwardOptions, "knownSessions"> & {
	knownSessions?: PortForwardSessionSummary[];
	invalidateQueries: InvalidatePortForwardQueries;
}): Promise<SavedPortForwardStartResult> {
	const result = await startSavedPortForwardCore({
		client,
		workspaceId,
		portForward,
		knownSessions: knownSessions ?? (await listPortForwards(client)),
		kubeconfigSource,
		updateSavedPortForward,
	});
	await invalidatePortForwardQueries(invalidateQueries);
	return result;
}

export async function startSavedPortForwards({
	client,
	workspace,
	portForwards = workspace?.portForwards ?? [],
	activeSessions,
	kubeconfigSource,
	updateSavedPortForward,
	invalidateQueries,
}: {
	client: TauriClient;
	workspace: SavedWorkspace | null;
	portForwards?: SavedPortForward[];
	activeSessions?: PortForwardSessionSummary[];
	kubeconfigSource?: string;
	updateSavedPortForward: UpdateSavedPortForward;
	invalidateQueries: InvalidatePortForwardQueries;
}): Promise<SavedPortForwardStartResult[]> {
	if (!workspace || portForwards.length === 0) return [];
	let knownSessions =
		activeSessions !== undefined
			? activeSessions
			: await listPortForwards(client);
	const results: SavedPortForwardStartResult[] = [];
	for (const portForward of portForwards) {
		const result = await startSavedPortForwardCore({
			client,
			workspaceId: workspace.id,
			portForward,
			knownSessions,
			kubeconfigSource,
			updateSavedPortForward,
		});
		results.push(result);
		if (result.session) knownSessions = [...knownSessions, result.session];
	}
	await invalidatePortForwardQueries(invalidateQueries);
	return results;
}

export type { SavedPortForward };
