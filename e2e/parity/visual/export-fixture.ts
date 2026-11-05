// Exports the browser-mock command responses as one JSON fixture for the GPUI
// visual driver, so both frontends render the same data.
// Usage: bun e2e/parity/visual/export-fixture.ts <output.json>
import { writeFile } from "node:fs/promises";
import { mcpMockStatus } from "../../../src/lib/mcp-dev-mocks";
import { LIVE_EVENTS } from "../../../src/lib/tauri-dev-mock-live";
import { createDevMockTauriClient } from "../../../src/lib/tauri-dev-mocks";
import type {
	ArgoApplicationSummary,
	ClusterContext,
	DiscoveredResourceKind,
	FluxDetectionSummary,
	FluxResourceSummary,
	IncidentCockpitSummary,
	KubeconfigSourcesSummary,
	NamespaceSummary,
	PodExecSessionSummary,
	PortForwardSessionSummary,
	RbacAccessReviewResult,
	RbacInspectionSummary,
	ResourceDetailsFull,
	ResourceEventSummary,
	ResourceSummary,
	ResourceTopology,
} from "../../../src/lib/types";
import { exportArgo } from "./export-argo";
import { exportDetailTabs } from "./export-detail-tabs";
import { exportGitOpsPage, exportHelm } from "./export-gitops-helm";
import { exportSessionControls } from "./export-session-controls";

// Browser mocks omit owner fields the Rust model requires; fill them with the
// values the web frontend infers (`owner.partial ?? confidence !== "inventory"`).
function rustShape(row: ResourceSummary): ResourceSummary {
	const owner = row.gitOpsOwner;
	return owner ? { ...row, gitOpsOwner: { provenance: "browser-mock", ...owner, partial: owner.partial ?? owner.confidence !== "inventory" } } : row;
}

const output = process.argv[2];
if (!output) throw new Error("Usage: export-fixture.ts <output.json>");
const client = createDevMockTauriClient();
const contexts = await client.invoke<ClusterContext[]>("list_kube_contexts");
const clusterContext = contexts.find((context) => context.isCurrent)?.name ?? contexts[0]?.name;
const namespaces = await client.invoke<NamespaceSummary[]>("list_namespaces", { clusterContext });
const kinds = await client.invoke<DiscoveredResourceKind[]>("list_resource_kinds", { clusterContext });
const resources = await client.invoke<ResourceSummary[]>("list_resources", { clusterContext });
const details: ResourceDetailsFull[] = [];
// Events per resource in the Rust `ResourceEventSummary` shape the mocks already use.
const events: { kind: string; name: string; namespace: string | null; events: ResourceEventSummary[] }[] = [];
for (const row of resources) {
	const detail = await client.invoke<ResourceDetailsFull>("get_resource_details", { clusterContext, kind: row.kind, name: row.name, namespace: row.namespace });
	details.push({ ...detail, summary: rustShape(detail.summary) });
	const rows = await client.invoke<ResourceEventSummary[]>("list_resource_events", { clusterContext, kind: row.kind, name: row.name, namespace: row.namespace });
	events.push({ kind: row.kind, name: row.name, namespace: row.namespace ?? null, events: rows });
}
// The web map reads every namespace (`namespaces: []`) for the All scope the views capture.
const topology = await client.invoke<ResourceTopology>("list_resource_topology", { clusterContext, namespaces: [], mode: "ownership" });
const rustGraph = (graph: ResourceTopology) => ({ ...graph, nodes: graph.nodes.map((node) => ({ ...node, summary: rustShape(node.summary) })) });
const rustTopology = rustGraph(topology);
const networkTopology = rustGraph(await client.invoke<ResourceTopology>("list_resource_topology", { clusterContext, namespaces: [], mode: "networkFlow" }));
const appUsage = await client.invoke("get_app_usage_metrics");
const metrics = await client.invoke("list_resource_metrics", { clusterContext, namespaces: [] });
// The overview's GitOps inventory: provider lists when their CRDs are detected, else null.
const argo = (await client.invoke<boolean>("detect_argocd", { clusterContext })) ? await client.invoke<ArgoApplicationSummary[]>("list_argocd_applications", { clusterContext }) : null;
const fluxDetection = await client.invoke<FluxDetectionSummary>("detect_flux", { clusterContext });
const flux = fluxDetection.detected ? (await Promise.all(fluxDetection.kinds.map((resourceKind) => client.invoke<FluxResourceSummary[]>("list_flux_resources", { clusterContext, resourceKind })))).flat() : null;
const gitops = { argo, flux, ...(await exportGitOpsPage(client, clusterContext, argo !== null)) };
const helm = await exportHelm(client, clusterContext);
// Mock incidents omit the states the Rust model requires; fill them as `incidentState` infers.
const cockpit = await client.invoke<IncidentCockpitSummary>("list_incident_cockpit", { clusterContext, requests: [] });
const incidents = {
	...cockpit,
	items: cockpit.items.map((item) => {
		const state = item.state ?? (item.severity === "restarted" ? "historical" : "active");
		return { ...item, state, resource: rustShape(item.resource), signals: item.signals.map((signal) => ({ ...signal, state: signal.state ?? state })) };
	}),
};
// The incident guide reads the ownership graph of a Pod's namespace.
const namespaceTopologies: Record<string, unknown> = {};
const topologyNamespaces = new Set<string>();
for (const item of incidents.items) {
	const namespace = item.resource.namespace;
	if (item.resource.kind === "Pod" && namespace) topologyNamespaces.add(namespace);
}
for (const app of argo ?? []) {
	for (const namespace of app.resourceNamespaces ?? []) topologyNamespaces.add(namespace);
	if (app.destinationNamespace) topologyNamespaces.add(app.destinationNamespace);
}
for (const namespace of topologyNamespaces) {
	namespaceTopologies[namespace] = rustGraph(await client.invoke<ResourceTopology>("list_resource_topology", { clusterContext, namespaces: [namespace], mode: "ownership" }));
}
const sources = await client.invoke<KubeconfigSourcesSummary>("get_kubeconfig_sources");
const liveSessions = {
	portForwards: await client.invoke<PortForwardSessionSummary[]>("list_port_forwards"),
	execSessions: await client.invoke<PodExecSessionSummary[]>("list_pod_exec_sessions"),
	sourceKey: sources.sourceKey,
	showSourceLabels: sources.showSourceLabels,
};
// The detail tabs the views capture read Deployment payments/payments-api.
const focus = resources.find((row) => row.kind === "Deployment" && row.name === "payments-api" && row.namespace === "payments");
if (!focus) throw new Error("Browser mocks lack Deployment payments/payments-api");
const detailTabs = await exportDetailTabs(client, [...resources, ...gitops.applications.map((app) => app.details.summary)], focus);
// The RBAC Cockpit per context, and the verifier's mock verdict per verb (an error for verbs it rejects).
const rbac: Record<string, RbacInspectionSummary> = {};
for (const context of contexts) {
	const summary = await client.invoke<RbacInspectionSummary>("list_rbac_inspection", { clusterContext: context.name });
	// Rust requires each coverage entry's namespace list, which the mocks omit.
	rbac[context.name] = { ...summary, coverage: summary.coverage.map((entry) => ({ ...entry, namespaces: entry.namespaces ?? [] })) };
}
const rbacReviews: { verb: string; result?: RbacAccessReviewResult; error?: string }[] = [];
for (const verb of ["get", "list", "watch", "create", "update", "patch", "delete", "impersonate"]) {
	const request = { clusterContext, identity: { kind: "group", group: "system:authenticated" }, target: { kind: "resource", verb, resource: "pods", apiGroup: "", namespace: null, subresource: null, name: null } };
	try { rbacReviews.push({ verb, result: await client.invoke<RbacAccessReviewResult>("review_rbac_access", { request }) }); }
	catch (error) { rbacReviews.push({ verb, error: error instanceof Error ? error.message : String(error) }); }
}
const sessionControls = await exportSessionControls(client, resources);
const argoConnected = await exportArgo(client, gitops.applications);
const secretReveals = [];
for (const row of resources.filter((row) => row.kind === "Secret")) {
    const request = { clusterContext: row.cluster, name: row.name, namespace: row.namespace, key: "token" };
    secretReveals.push({ request, encoded: await client.invoke<string>("reveal_secret_data_value", request) });
}
await writeFile(output, JSON.stringify({ mcp: mcpMockStatus(), argoConnected, secretReveals, sessionControls, liveEvents: LIVE_EVENTS, contexts, namespaces, kinds, resources: resources.map(rustShape), details, events, topology: rustTopology, networkTopology, metrics, appUsage, namespaceTopologies, gitops, helm, incidents, liveSessions, detailTabs, rbac, rbacReviews, kubeconfig: sources }, null, "\t"));
console.log(`fixture: ${resources.length} resources, ${details.length} details -> ${output}`);
