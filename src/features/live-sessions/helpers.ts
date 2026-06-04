import type {
	PortForwardRequest,
	PortForwardSessionSummary,
	ResourceSummary,
} from "@/lib/types";
import { normalizeKubeconfigEnvVar } from "@/lib/settings";
import type {
	SavePortForwardInput,
	SavedPortForward,
} from "@/lib/workspaces";

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
	kubeconfigEnvVar?: string,
): boolean {
	return (
		normalizeKubeconfigEnvVar(session.kubeconfigEnvVar) ===
			normalizeKubeconfigEnvVar(kubeconfigEnvVar) &&
		session.clusterContext === resource.cluster &&
		session.namespace === resource.namespace &&
		session.targetKind === resource.kind &&
		session.targetName === resource.name
	);
}

export function savedPortForwardLabel(portForward: SavedPortForward): string {
	return (
		portForward.label?.trim() ||
		`${portForward.namespace}/${portForward.serviceName}:${portForward.servicePort}`
	);
}

export function savedPortForwardToRequest(
	portForward: SavedPortForward,
	kubeconfigEnvVar?: string,
): PortForwardRequest {
	return {
		clusterContext: portForward.clusterContext,
		kubeconfigEnvVar,
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
	kubeconfigEnvVar?: string,
): boolean {
	return (
		normalizeKubeconfigEnvVar(session.kubeconfigEnvVar) ===
			normalizeKubeconfigEnvVar(kubeconfigEnvVar) &&
		session.clusterContext === portForward.clusterContext &&
		session.namespace === portForward.namespace &&
		session.targetKind === "Service" &&
		session.targetName === portForward.serviceName &&
		session.remotePort === portForward.servicePort &&
		(portForward.localPort === undefined ||
			session.localPort === portForward.localPort)
	);
}

export interface ServicePortOption {
	name?: string;
	port: number;
	targetPort?: string;
	protocol?: string;
}

export interface PortForwardFormValues {
	remotePort: string;
	localPort: string;
}

export interface ParsedPortForwardForm {
	remotePort: number;
	localPort?: number;
}

export interface SavedPortForwardFormValues {
	clusterContext: string;
	namespace: string;
	serviceName: string;
	servicePort: string;
	localPort: string;
	label: string;
}

function parsePort(value: string, label: string): number | string {
	const trimmed = value.trim();
	if (!trimmed) {
		return `${label} is required`;
	}
	if (!/^\d+$/.test(trimmed)) {
		return `${label} must be a number`;
	}
	const port = Number(trimmed);
	if (!Number.isInteger(port) || port < 1 || port > 65535) {
		return `${label} must be between 1 and 65535`;
	}
	return port;
}

function yamlIndent(line: string): number {
	return line.length - line.trimStart().length;
}

function unquoteYamlScalar(value: string): string {
	const trimmed = value.trim();
	if (
		(trimmed.startsWith('"') && trimmed.endsWith('"')) ||
		(trimmed.startsWith("'") && trimmed.endsWith("'"))
	) {
		return trimmed.slice(1, -1);
	}
	return trimmed;
}

function parseYamlKeyValue(text: string): [string, string] | null {
	const match = text.match(/^([A-Za-z][\w-]*):\s*(.*?)\s*$/);
	if (!match) return null;
	return [match[1], unquoteYamlScalar(match[2])];
}

function applyServicePortField(
	port: Partial<ServicePortOption>,
	key: string,
	value: string,
): void {
	if (key === "port" && /^\d+$/.test(value)) {
		port.port = Number(value);
	} else if (key === "name" && value) {
		port.name = value;
	} else if (key === "targetPort" && value) {
		port.targetPort = value;
	} else if (key === "protocol" && value) {
		port.protocol = value;
	}
}

export function extractServicePortOptions(yaml: string | undefined): ServicePortOption[] {
	if (!yaml) return [];
	const ports: Partial<ServicePortOption>[] = [];
	let specIndent: number | null = null;
	let portsIndent: number | null = null;
	let current: Partial<ServicePortOption> | null = null;

	for (const line of yaml.split(/\r?\n/)) {
		const trimmed = line.trim();
		if (!trimmed || trimmed.startsWith("#")) continue;
		const indent = yamlIndent(line);

		if (specIndent === null) {
			if (trimmed === "spec:") specIndent = indent;
			continue;
		}
		if (indent <= specIndent && trimmed !== "spec:") break;

		if (portsIndent === null) {
			if (trimmed === "ports:") portsIndent = indent;
			continue;
		}
		if (indent < portsIndent || (indent === portsIndent && !trimmed.startsWith("-"))) {
			break;
		}

		if (trimmed === "-" || trimmed.startsWith("- ")) {
			current = {};
			ports.push(current);
			const inline = trimmed.slice(1).trim();
			if (inline) {
				const parsed = parseYamlKeyValue(inline);
				if (parsed) applyServicePortField(current, parsed[0], parsed[1]);
			}
			continue;
		}

		if (!current) continue;
		const parsed = parseYamlKeyValue(trimmed);
		if (parsed) applyServicePortField(current, parsed[0], parsed[1]);
	}

	return ports
		.filter((port): port is ServicePortOption => {
			const protocol = port.protocol?.toUpperCase() ?? "TCP";
			return (
				typeof port.port === "number" &&
				Number.isInteger(port.port) &&
				port.port > 0 &&
				protocol === "TCP"
			);
		})
		.toSorted((a, b) => a.port - b.port || (a.name ?? "").localeCompare(b.name ?? ""));
}

export function parsePortForwardForm(
	values: PortForwardFormValues,
	options: { remotePortLabel?: string } = {},
): ParsedPortForwardForm | string {
	const remotePort = parsePort(
		values.remotePort,
		options.remotePortLabel ?? "Remote port",
	);
	if (typeof remotePort === "string") return remotePort;

	const localPortText = values.localPort.trim();
	if (!localPortText) return { remotePort };

	const localPort = parsePort(localPortText, "Local port");
	if (typeof localPort === "string") return localPort;
	if (localPort < 1024) return "Local port must be 1024 or higher";

	return { remotePort, localPort };
}

export function parseSavedPortForwardForm(
	values: SavedPortForwardFormValues,
): SavePortForwardInput | string {
	const clusterContext = values.clusterContext.trim();
	const namespace = values.namespace.trim();
	const serviceName = values.serviceName.trim();
	const label = values.label.trim();
	if (!clusterContext) return "Cluster context is required";
	if (!namespace) return "Namespace is required";
	if (!serviceName) return "Service name is required";

	const servicePort = parsePort(values.servicePort, "Service port");
	if (typeof servicePort === "string") return servicePort;

	const localPortText = values.localPort.trim();
	if (!localPortText) {
		return {
			clusterContext,
			namespace,
			serviceName,
			servicePort,
			localPort: undefined,
			label: label || undefined,
		};
	}

	const localPort = parsePort(localPortText, "Local port");
	if (typeof localPort === "string") return localPort;
	if (localPort < 1024) return "Local port must be 1024 or higher";

	return {
		clusterContext,
		namespace,
		serviceName,
		servicePort,
		localPort,
		label: label || undefined,
	};
}
