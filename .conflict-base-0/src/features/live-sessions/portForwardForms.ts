import type { SavePortForwardInput } from "@/lib/workspaces";

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
	if (!trimmed) return `${label} is required`;
	if (!/^\d+$/.test(trimmed)) return `${label} must be a number`;
	const port = Number(trimmed);
	if (!Number.isInteger(port) || port < 1 || port > 65535) {
		return `${label} must be between 1 and 65535`;
	}
	return port;
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
	if (key === "port" && /^\d+$/.test(value)) port.port = Number(value);
	else if (key === "name" && value) port.name = value;
	else if (key === "targetPort" && value) port.targetPort = value;
	else if (key === "protocol" && value) port.protocol = value;
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
		const indent = line.length - line.trimStart().length;

		if (specIndent === null) {
			if (trimmed === "spec:") specIndent = indent;
			continue;
		}
		if (indent <= specIndent && trimmed !== "spec:") break;

		if (portsIndent === null) {
			if (trimmed === "ports:") portsIndent = indent;
			continue;
		}
		if (indent < portsIndent || (indent === portsIndent && !trimmed.startsWith("-"))) break;

		if (trimmed === "-" || trimmed.startsWith("- ")) {
			current = {};
			ports.push(current);
			const parsed = parseYamlKeyValue(trimmed.slice(1).trim());
			if (parsed) applyServicePortField(current, parsed[0], parsed[1]);
			continue;
		}

		if (!current) continue;
		const parsed = parseYamlKeyValue(trimmed);
		if (parsed) applyServicePortField(current, parsed[0], parsed[1]);
	}

	return ports
		.filter((port): port is ServicePortOption => {
			const protocol = port.protocol?.toUpperCase() ?? "TCP";
			return typeof port.port === "number" && port.port > 0 && protocol === "TCP";
		})
		.toSorted((a, b) => a.port - b.port || (a.name ?? "").localeCompare(b.name ?? ""));
}

export function parsePortForwardForm(
	values: PortForwardFormValues,
	options: { remotePortLabel?: string } = {},
): ParsedPortForwardForm | string {
	const remotePort = parsePort(values.remotePort, options.remotePortLabel ?? "Remote port");
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
