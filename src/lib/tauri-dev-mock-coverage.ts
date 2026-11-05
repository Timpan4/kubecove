import type { HelmReleaseList, IncidentCockpitSummary, PodLogStreamRequest, RbacInspectionSummary, ResourceTopology, StreamMessage, YamlApplyPreview } from "./types";

declare global { var __kubecoveParityCoverage: string | undefined; }
let script = globalThis.__kubecoveParityCoverage ?? "base";
const pending: (() => void)[] = [];
let logSender: (() => void) | undefined;
export function coverageNextLog(streamId: string): StreamMessage {
	return { type: "logLine", streamId, line: "2026-06-29T10:01:00Z Newest controlled line" };
}
export function registerCoverageLogStream(streamId: string, send: (message: StreamMessage) => void): void {
	logSender = () => send(coverageNextLog(streamId));
}
export function emitCoverageLog(): void {
	if (!logSender) throw new Error("No controlled log stream");
	logSender();
}
export function configureCoverage(next: string): void {
	script = next;
	for (const release of pending.splice(0)) release();
}
export function coverageYamlPreview(forceConflicts: boolean | undefined, preview: YamlApplyPreview): YamlApplyPreview {
	if (script === "conflict" && !forceConflicts) throw { kind: "fieldManagerConflict", message: "Controlled field ownership conflict" };
	return preview;
}
export function coverageLogs(request: Partial<PodLogStreamRequest>, streamId: string): StreamMessage[] | null {
	if (!script.startsWith("logs-")) return null;
	const messages: StreamMessage[] = [{ type: "started", streamId, label: "Controlled log stream" }, { type: "status", streamId, status: "connected", message: "Controlled log stream connected" }];
	if (script === "logs-failed") messages.push({ type: "error", streamId, message: "Controlled log stream failure" });
	if (script === "logs-options") messages.push({ type: "logLine", streamId, line: `2026-06-29T10:00:00Z Selected ${request.container} tail ${request.tailLines} since ${request.sinceSeconds ?? "any"}` });
	if (script === "logs-long") messages.push({ type: "logLine", streamId, line: `2026-06-29T10:00:00Z ${"Long wrapped evidence ".repeat(40)}` });
	if (script === "logs-stream") for (let index = 0; index < (request.tailLines ?? 200); index++) messages.push({ type: "logLine", streamId, line: `2026-06-29T10:00:00Z Earlier controlled line ${index + 1}` });
	return messages;
}
const commands = new Set(["get_resource_details", "list_resource_topology", "list_helm_releases", "get_helm_release_details", "list_rbac_inspection", "list_incident_cockpit", "get_kubeconfig_sources", "set_kubeconfig_env_var", "list_port_forwards", "list_pod_exec_sessions"]);
export async function beforeCoverageRead(command: string): Promise<void> {
	if (!commands.has(command)) return;
	if (script === "detail-loading" && command === "get_helm_release_details") await new Promise<void>((resolve) => pending.push(resolve));
	if (script === "enrichment-loading" && (command === "get_resource_details" || command === "list_resource_topology")) await new Promise<void>((resolve) => pending.push(resolve));
	if (script === "enrichment-failed" && (command === "get_resource_details" || command === "list_resource_topology")) throw new Error("Controlled enrichment failure");
	if (script === "startup-failed" && command === "get_kubeconfig_sources") throw new Error("Controlled source read failure");
	if (script === "loading" && command !== "get_kubeconfig_sources") await new Promise<void>((resolve) => pending.push(resolve));
	if (script === "failed") throw new Error("Controlled inspection failure");
}
export function coverageHelm(data: HelmReleaseList): HelmReleaseList {
	if (script === "empty") return { ...data, releases: [] };
	if (script === "partial") return { ...data, warnings: ["Controlled Helm storage warning"] };
	return data;
}
export function coverageRbac(data: RbacInspectionSummary): RbacInspectionSummary {
	if (script === "many") {
		const role = data.roles[0];
		if (!role) throw new Error("RBAC fixture needs a template Role");
		return { ...data, roles: Array.from({ length: 51 }, (_, index) => ({ ...role, name: `coverage-role-${String(index + 1).padStart(3, "0")}` })) };
	}
	if (script === "empty") return { ...data, roles: [], clusterRoles: [], roleBindings: [], clusterRoleBindings: [], serviceAccounts: [] };
	if (script === "partial") return { ...data, warnings: ["Controlled RBAC coverage warning"] };
	return data;
}
export function coverageIncidents(data: IncidentCockpitSummary): IncidentCockpitSummary {
	if (script === "attention-only") return { ...data, items: data.items.map((item): IncidentCockpitSummary["items"][number] => ({ ...item, severity: "attention" })) };
	if (script === "empty") return { ...data, items: [] };
	if (script === "partial") return { ...data, warnings: ["Controlled incident coverage warning"] };
	return data;
}
export function coverageTopology(data: ResourceTopology): ResourceTopology {
	if (script === "partial") return { ...data, warnings: ["Controlled topology warning"] };
	return data;
}
export function coverageSessions<Item extends { kubeconfigSourceKey?: string }>(items: Item[]): Item[] {
	if (script === "empty") return [];
	if (script === "foreign") return items.map((item) => ({ ...item, kubeconfigSourceKey: "foreign-coverage-source" }));
	return items;
}
export function coverageStopForward(id: string, sessions: { id: string }[]): void {
	if (script !== "stop-failed") return;
	if (!sessions.some((session) => session.id === id)) throw new Error("Unknown controlled forward");
	throw new Error("Controlled stop failure");
}
