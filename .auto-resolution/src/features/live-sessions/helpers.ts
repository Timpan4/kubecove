import type {
	PortForwardRequest,
	PortForwardSessionSummary,
	ResourceSummary,
} from "@/lib/types";
import { normalizeKubeconfigEnvVar } from "@/lib/settings";
import type { SavedPortForward } from "@/lib/workspaces";

export function portForwardErrorMessage(error: unknown): string {
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

export function portForwardLocalUrl(session: PortForwardSessionSummary): string {
	return session.localUrl || `http://${session.localAddress}:${session.localPort}`;
}

export function portForwardSessionKey(
	session: Pick<
		PortForwardSessionSummary,
		| "clusterContext"
		| "namespace"
		| "targetKind"
		| "targetName"
		| "remotePort"
		| "localPort"
	>,
): string {
	return [
		session.clusterContext,
		session.namespace,
		session.targetKind,
		session.targetName,
		session.remotePort,
		session.localPort,
	].join(":");
}

export function sortPortForwardSessions(
	sessions: PortForwardSessionSummary[],
): PortForwardSessionSummary[] {
	return sessions.toSorted((a, b) => {
		const targetCompare = portForwardSessionKey(a).localeCompare(
			portForwardSessionKey(b),
		);
		if (targetCompare !== 0) return targetCompare;
		return a.id.localeCompare(b.id);
	});
}

export function isReusablePortForwardSession(
	session: Pick<PortForwardSessionSummary, "status">,
): boolean {
	return session.status === "listening" || session.status === "connected";
}

export function isPortForwardForResource(
	session: PortForwardSessionSummary,
	resource: ResourceSummary,
	kubeconfigSource?: string,
): boolean {
	return (
		portForwardSessionMatchesKubeconfigSource(session, kubeconfigSource) &&
		session.clusterContext === resource.cluster &&
		session.namespace === resource.namespace &&
		session.targetKind === resource.kind &&
		session.targetName === resource.name
	);
}

export function portForwardSessionMatchesKubeconfigSource(
	session: PortForwardSessionSummary,
	kubeconfigSource?: string,
): boolean {
	return sessionKubeconfigSource(session) === normalizeKubeconfigSource(kubeconfigSource);
}

export function savedPortForwardLabel(portForward: SavedPortForward): string {
	return (
		portForward.label?.trim() ||
		`${portForward.namespace}/${portForward.serviceName}:${portForward.servicePort}`
	);
}

export function savedPortForwardToRequest(
	portForward: SavedPortForward,
	kubeconfigSource?: string,
): PortForwardRequest {
	return {
		clusterContext: portForward.clusterContext,
		kubeconfigEnvVar: portForwardRequestKubeconfigEnvVar(kubeconfigSource),
		namespace: portForward.namespace,
		localPort: portForward.localPort,
		targetKind: "Service",
		targetName: portForward.serviceName,
		remotePort: portForward.servicePort,
	};
}

export function portForwardSessionToRequest(
	session: PortForwardSessionSummary,
): PortForwardRequest {
	return {
		clusterContext: session.clusterContext,
		kubeconfigEnvVar: session.kubeconfigEnvVar,
		namespace: session.namespace,
		localPort: session.localPort,
		targetKind: session.targetKind === "Service" ? "Service" : "Pod",
		targetName: session.targetName,
		remotePort: session.remotePort,
	};
}

export function savedPortForwardMatchesSession(
	portForward: SavedPortForward,
	session: PortForwardSessionSummary,
	kubeconfigSource?: string,
): boolean {
	return (
		sessionKubeconfigSource(session) === normalizeKubeconfigSource(kubeconfigSource) &&
		session.clusterContext === portForward.clusterContext &&
		session.namespace === portForward.namespace &&
		session.targetKind === "Service" &&
		session.targetName === portForward.serviceName &&
		session.remotePort === portForward.servicePort &&
		(portForward.localPort === undefined ||
			session.localPort === portForward.localPort)
	);
}

export function savedPortForwardLocalPortConflict(
	portForward: SavedPortForward,
	sessions: PortForwardSessionSummary[],
): PortForwardSessionSummary | null {
	if (portForward.localPort === undefined) return null;
	return (
		sessions.find((session) => session.localPort === portForward.localPort) ??
		null
	);
}

function normalizeKubeconfigSource(source?: string): string {
	if (source?.startsWith("kubeconfigSource=")) return source;
	return `kubeconfigEnv=${normalizeKubeconfigEnvVar(source)}`;
}

function sessionKubeconfigSource(session: PortForwardSessionSummary): string {
	return (
		session.kubeconfigSourceKey ??
		`kubeconfigEnv=${normalizeKubeconfigEnvVar(session.kubeconfigEnvVar)}`
	);
}

export function portForwardRequestKubeconfigEnvVar(
	source?: string,
): string | undefined {
	return source?.startsWith("kubeconfigSource=") ? undefined : source;
}
