import type { TauriClient } from "../../../src/lib/tauri";
import { configureCoverage, coverageHelm, coverageIncidents, coverageNextLog, coverageRbac, coverageSessions, coverageTopology } from "../../../src/lib/tauri-dev-mock-coverage";
import type { HelmReleaseList, IncidentCockpitSummary, PodExecSessionSummary, PortForwardSessionSummary, RbacInspectionSummary, ResourceTopology } from "../../../src/lib/types";
import { streamMessages } from "./export-detail-tabs";

export async function exportCoverageLogs(client: TauriClient) {
	const cases = [];
	try {
		for (const script of ["logs-empty", "logs-failed", "logs-options", "logs-long", "logs-stream"]) {
			configureCoverage(script);
			for (const container of ["app", "sidecar"]) for (const tailLines of [200, 2]) for (const sinceSeconds of [undefined, 300]) {
				const request = { clusterContext: "mock-dev", namespace: "payments", podName: "payments-api-7d9c9b7f8d-k2r9p", container, tailLines, sinceSeconds, batchLines: true };
				const messages = await streamMessages(client, "start_pod_log_stream", request);
				const started = messages.find((message) => message.type === "started");
				cases.push({ script, request, messages, next: script === "logs-stream" && started ? coverageNextLog(started.streamId) : undefined });
			}
		}
	} finally { configureCoverage("base"); }
	return cases;
}

export async function exportCoverage(client: TauriClient, base: { helm: HelmReleaseList; rbac: RbacInspectionSummary; incidents: IncidentCockpitSummary; topology: ResourceTopology; sessions: { portForwards: PortForwardSessionSummary[]; execSessions: PodExecSessionSummary[] } }) {
	const responses: Record<string, Record<string, { result?: object; error?: string }>> = {};
	try {
		for (const script of ["empty", "partial", "failed", "many", "foreign", "enrichment-failed", "attention-only", "stop-failed"]) {
			configureCoverage(script);
			const entries: Record<string, { result?: object; error?: string }> = {};
			for (const command of ["list_resource_topology", "list_helm_releases", "get_helm_release_details", "list_rbac_inspection", "list_incident_cockpit", "get_kubeconfig_sources", "set_kubeconfig_env_var", "list_port_forwards", "list_pod_exec_sessions"]) {
				try { entries[command] = { result: await client.invoke<object>(command, { clusterContext: "mock-dev", namespaces: [], requests: [], name: "metrics-gateway", namespace: "platform", envVar: "KUBECONFIG" }) }; }
				catch (error) { entries[command] = { error: error instanceof Error ? error.message : String(error) }; }
			}
			if (script === "enrichment-failed" || script === "stop-failed") {
				const command = script === "stop-failed" ? "stop_port_forward" : "get_resource_details";
				try { await client.invoke(command, { sessionId: "mock-pf-grafana", clusterContext: "mock-dev", kind: "Pod", name: "operations-crashloop-7f468cd8b8-lab01", namespace: "operations" }); }
				catch (error) { entries[command] = { error: error instanceof Error ? error.message : String(error) }; }
			}
			responses[script] = entries;
			if (script !== "failed") {
				entries.list_helm_releases = { result: coverageHelm(base.helm) };
				entries.list_rbac_inspection = { result: coverageRbac(base.rbac) };
				entries.list_incident_cockpit = { result: coverageIncidents(base.incidents) };
				if (script !== "enrichment-failed") entries.list_resource_topology = { result: coverageTopology(base.topology) };
				entries.list_port_forwards = { result: coverageSessions(base.sessions.portForwards) };
				entries.list_pod_exec_sessions = { result: coverageSessions(base.sessions.execSessions) };
			}
		}
	} finally { configureCoverage("base"); }
	return responses;
}
