import type { PortForwardSessionSummary, ResourceSummary } from "@/lib/types";

export function portForwardLocalUrl(session: PortForwardSessionSummary): string {
	return session.localUrl || `http://${session.localAddress}:${session.localPort}`;
}

export function portForwardSessionKey(
	session: Pick<
		PortForwardSessionSummary,
		"clusterContext" | "namespace" | "podName" | "remotePort" | "localPort"
	>,
): string {
	return [
		session.clusterContext,
		session.namespace,
		session.podName,
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

export function isPortForwardForResource(
	session: PortForwardSessionSummary,
	resource: ResourceSummary,
): boolean {
	return (
		resource.kind === "Pod" &&
		session.clusterContext === resource.cluster &&
		session.namespace === resource.namespace &&
		session.podName === resource.name
	);
}

export interface PortForwardFormValues {
	remotePort: string;
	localPort: string;
}

export interface ParsedPortForwardForm {
	remotePort: number;
	localPort?: number;
}

function parsePort(value: string, label: string): number | string {
	const trimmed = value.trim();
	if (!/^\d+$/.test(trimmed)) {
		return `${label} must be a number`;
	}
	const port = Number(trimmed);
	if (!Number.isInteger(port) || port < 1 || port > 65535) {
		return `${label} must be between 1 and 65535`;
	}
	return port;
}

export function parsePortForwardForm(
	values: PortForwardFormValues,
): ParsedPortForwardForm | string {
	const remotePort = parsePort(values.remotePort, "Remote port");
	if (typeof remotePort === "string") return remotePort;

	const localPortText = values.localPort.trim();
	if (!localPortText) return { remotePort };

	const localPort = parsePort(localPortText, "Local port");
	if (typeof localPort === "string") return localPort;
	if (localPort < 1024) return "Local port must be 1024 or higher";

	return { remotePort, localPort };
}
