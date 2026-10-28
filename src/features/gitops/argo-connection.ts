import { argoEndpointIdentity } from "@/lib/argo-connection-policy";
import type { SavedArgoProfile } from "@/lib/settings";
import { connectArgoServer, getArgoConnectionStatus } from "@/lib/tauri";
import type { TauriClient } from "@/lib/tauri-runtime";
import type {
	ArgoConnectionStatus,
	ArgoServerCapability,
	ArgoServerEndpoint,
} from "@/lib/types";

/** The workspace an Argo CD connection profile belongs to. */
export interface ArgoWorkspaceScope {
	clusterContext: string;
	workspaceId: string;
	kubeconfigSourceKey?: string;
}

export function argoEndpointUrl(endpoint: ArgoServerEndpoint): string {
	return endpoint.kind === "externalHttps"
		? endpoint.url
		: `${endpoint.scheme}://${endpoint.serviceName}.${endpoint.namespace}.svc${endpoint.rootPath ?? ""}`;
}

export function argoEndpointLabel(endpoint: ArgoServerEndpoint): string {
	return endpoint.kind === "externalHttps"
		? endpoint.url
		: `${endpoint.namespace}/${endpoint.serviceName}:${endpoint.servicePort} (${endpoint.scheme.toUpperCase()})`;
}

export function argoProfileId(scope: ArgoWorkspaceScope, endpoint: ArgoServerEndpoint): string {
	return `argo:${scope.workspaceId}:${scope.clusterContext}:${scope.kubeconfigSourceKey ?? "global"}:${argoEndpointIdentity(endpoint)}`;
}

export function savedArgoProfile(status: ArgoConnectionStatus): SavedArgoProfile | null {
	const profile = status.profile;
	if (!profile) return null;
	return {
		id: profile.id,
		endpoint: profile.endpoint,
		clusterContext: profile.clusterContext ?? undefined,
		workspaceId: profile.workspaceId ?? undefined,
		kubeconfigSourceKey: profile.kubeconfigSourceKey,
		rememberCredential: profile.rememberCredential,
	};
}

/** Tunnel endpoints that can be connected, in the order discovery ranked them. */
export function connectableArgoServers(
	capabilities: readonly ArgoServerCapability[] | undefined,
): ArgoServerCapability[] {
	return (capabilities ?? []).filter(
		(server) => server.endpoint?.kind === "serviceTunnel" && !server.unavailableReason,
	);
}

/** `ab12…` as `AB:12:…`, the form certificate viewers show. */
export function formatCertificateSha256(fingerprint: string): string {
	return fingerprint.toUpperCase().match(/.{2}/g)?.join(":") ?? fingerprint;
}

/**
 * Reconnects with the credential remembered in the native keyring. Tunnel profiles confirm
 * only the Service; the backend accepts that solely for a remembered credential and reuses
 * the certificate pinned with it (ADR 0017).
 */
export function reconnectRememberedArgoProfile(
	client: TauriClient,
	scope: ArgoWorkspaceScope,
	profile: SavedArgoProfile,
): Promise<ArgoConnectionStatus> {
	const endpoint = profile.endpoint;
	return connectArgoServer(client, {
		id: profile.id,
		serverUrl: argoEndpointUrl(endpoint),
		endpoint,
		insecureTls: false,
		rememberCredential: true,
		clusterContext: scope.clusterContext,
		kubeconfigEnvVar: scope.kubeconfigSourceKey,
		workspaceId: scope.workspaceId,
		confirmedTarget:
			endpoint.kind === "serviceTunnel"
				? { namespace: endpoint.namespace, serviceName: endpoint.serviceName, podName: null }
				: undefined,
	});
}

/** Reconnects each remembered profile that is not connected; failures stay for manual reconnect. */
export async function reconnectRememberedArgoProfiles(
	client: TauriClient,
	scope: ArgoWorkspaceScope,
	profiles: readonly SavedArgoProfile[],
): Promise<void> {
	await Promise.allSettled(
		profiles
			.filter((profile) => profile.rememberCredential)
			.map(async (profile) => {
				if ((await getArgoConnectionStatus(client, profile.id)).connected) return;
				await reconnectRememberedArgoProfile(client, scope, profile);
			}),
	);
}
