import { get, writable } from "svelte/store";
import {
	ARGO_SYNC_DEFAULTS,
	type ArgoDiffView,
	type ArgoPrototypePhase,
	type ArgoSyncSettings,
} from "./argo-workspace-model";

export type ArgoDashboardTab = "overview" | "resources" | "history" | "diff";
export type ArgoHeaderVariation = "briefing" | "signals" | "flow";

export interface ArgoWorkspaceSession {
	dashboardTab: ArgoDashboardTab;
	headerVariation: ArgoHeaderVariation;
	phase: ArgoPrototypePhase;
	healthStatus: string;
	syncStatus: string;
	message: string;
	synced: boolean;
	syncDraft: ArgoSyncSettings;
	selectedResourceKey: string | null;
	selectedHistoryId: number | null;
	diffView: ArgoDiffView;
}

const sessions = writable<Record<string, ArgoWorkspaceSession>>({});

export const argoWorkspaceSessions = { subscribe: sessions.subscribe };

export function argoWorkspaceSessionKey(
	clusterContext: string,
	workspaceId: string,
	namespace: string | null | undefined,
	name: string,
): string {
	return `${workspaceId}:${clusterContext}:${namespace ?? ""}:${name}`;
}

export function createArgoWorkspaceSession(): ArgoWorkspaceSession {
	return {
		dashboardTab: "overview",
		headerVariation: "briefing",
		phase: "idle",
		healthStatus: "Degraded",
		syncStatus: "OutOfSync",
		message: "Two resources differ from the configured revision.",
		synced: false,
		syncDraft: { ...ARGO_SYNC_DEFAULTS },
		selectedResourceKey: null,
		selectedHistoryId: 18,
		diffView: "changes",
	};
}

export function getArgoWorkspaceSession(key: string): ArgoWorkspaceSession | null {
	return get(sessions)[key] ?? null;
}

export function ensureArgoWorkspaceSession(key: string): void {
	sessions.update((current) =>
		current[key] ? current : { ...current, [key]: createArgoWorkspaceSession() },
	);
}

export function patchArgoWorkspaceSession(
	key: string,
	patch: Partial<ArgoWorkspaceSession>,
): void {
	sessions.update((current) => ({
		...current,
		[key]: { ...(current[key] ?? createArgoWorkspaceSession()), ...patch },
	}));
}

export function resetArgoWorkspaceSession(key: string): void {
	sessions.update((current) => ({ ...current, [key]: createArgoWorkspaceSession() }));
}
