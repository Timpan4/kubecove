import type { Channel, InvokeOptions } from "@tauri-apps/api/core";
import { stringify } from "yaml";
import { connectRemoteMock, disconnectRemoteMock, remoteMockStatus, saveWorkspaceMock, selectedDialogPath, selectedWorkspaceFile } from "./settings-integration-mocks";
import { argoApplicationSummary, argoApplicationYaml, argoMockHandlers, customResourceKinds, presentCustomResourceKinds } from "./tauri-dev-mock-argo";
import { type MockArgs, type MockHandler, type MockResult, readMockArgs } from "./tauri-dev-mock-contract";
import { beforeCoverageRead, coverageHelm, coverageIncidents, coverageLogs, coverageRbac, coverageSessions, coverageStopForward, coverageTopology, coverageYamlPreview, registerCoverageLogStream } from "./tauri-dev-mock-coverage";
import {
	argoApps,
	deploymentRevisions,
	dockerResources,
	helmReleases,
	mockContexts,
	namespaces,
	networkIngressTargets,
	networkServiceFallbackSelectors,
	networkServicePodMap,
	now,
	ownershipDependencyHints,
	res,
	resources,
	source,
} from "./tauri-dev-mock-data";
import { withDetailStatusFixture } from "./tauri-dev-mock-detail-status";
import { evictedWorkerName, evictedWorkerStatus } from "./tauri-dev-mock-incidents";
import { addKubeconfigPathsMock, reorderKubeconfigPathsMock } from "./tauri-dev-mock-kubeconfig";
import { lintMockYaml } from "./tauri-dev-mock-lint";
import { liveEvents, liveResources, mockListVersion, registerMockWatch, stopMockWatch } from "./tauri-dev-mock-live";
import { operationMockHandlers } from "./tauri-dev-mock-operations";
import { pageReadResult } from "./tauri-dev-mock-pages";
import { rbacInspectionMock, rbacReviewMock } from "./tauri-dev-mock-rbac";
import { cleanupMockSessions, listMockExecs, listMockForwards, mockExecInput, resizeMockExec, startMockExec, startMockForward, stopMockExec, stopMockForward } from "./tauri-dev-mock-session-controls";
import { shellKubeconfigSourcesRead, shellRead, shellWithoutRemoved } from "./tauri-dev-mock-shell";
import { mockTiming } from "./tauri-dev-mock-timing";
import { createMockChannel, type TauriClient } from "./tauri-runtime";
import type {
	AppUsageMetrics,
	BackendDiagnosticEvent,
	HelmReconciliationResource,
	HelmReleaseDetails,
	HelmReleaseList,
	HelmReleaseReconciliation,
	HelmReleaseSummary,
	IncidentCockpitSummary,
	IncidentSeverity,
    KubeconfigSourcesSummary,
	ResourceDetailsFull,
	ResourceEventSummary,
	ResourceListRequest,
	ResourceMetricsSummary,
	ResourceSummary,
	ResourceTopology,
	StreamMessage,
	YamlApplyPreview,
} from "./types";
import { formatYamlDocument } from "./yamlFormat";


let failedWorkspaceContext: string | undefined;
export function failWorkspaceContext(context: string) { failedWorkspaceContext = context; }



export function createDevMockTauriClient(): TauriClient {
	return {
		createChannel: (onMessage) => createMockChannel(onMessage),
		invoke: async <T, Args extends object = object>(
			cmd: string,
			args?: Args,
			options?: InvokeOptions,
		): Promise<T> => {
			await delay(35);
			await beforeCoverageRead(cmd);
            // The same typed seam as the wrappers; live channels have a different message contract.
            const live = args as { request: import("./types").PortForwardRequest & import("./types").PodExecSessionRequest & import("./types").LiveSessionCleanupRequest; sessionId: string; data: string; size: import("./types").PodExecTerminalSize; channel: Pick<Channel<import("./types").PodExecSessionMessage>, "onmessage"> } | undefined;
            if (cmd === "start_pod_port_forward" && live) return startMockForward(live.request) as T;
            if (cmd === "start_pod_exec_session" && live) return startMockExec(live.request, live.channel) as T;
            if (cmd === "write_pod_exec_stdin" && live) return mockExecInput(live.sessionId, live.data) as T;
            if (cmd === "resize_pod_exec_terminal" && live) return resizeMockExec(live.sessionId, live.size) as T;
            if (cmd === "stop_pod_exec_session" && live) return stopMockExec(live.sessionId) as T;
            if (cmd === "stop_port_forward" && live) { coverageStopForward(live.sessionId, listMockForwards()); return stopMockForward(live.sessionId) as T; }
            if (cmd === "stop_live_sessions_outside_scope" && live) return cleanupMockSessions(live.request) as T;
			const handler = findMockHandler(cmd);
			if (!handler) throw new Error(`No browser dev mock for command: ${cmd}`);
			// SAFETY: caller's typed wrapper owns result T; each mock handler mirrors that command.
			return await mockTiming.read(cmd, readMockArgs(args), async () => pageReadResult(cmd, await handler(readMockArgs(args), options))) as T;
		},
	};
}

// The page's kubeconfig settings, so saved values read back like the desktop settings file.
const kubeconfigSource = { ...source };
const kubeconfigSources = () => shellKubeconfigSourcesRead({ ...kubeconfigSource });
export function restoreMockKubeconfigSources(sources: KubeconfigSourcesSummary): void {
    Object.assign(kubeconfigSource, sources);
}

const handlers = {
	remote_connect: (args) => connectRemoteMock(args.serverUrl ?? ""),
	remote_disconnect: disconnectRemoteMock,
	remote_connection_status: remoteMockStatus,
	remote_invoke: (args): MockResult | Promise<MockResult> => {
		if (!remoteMockStatus()?.connected) throw new Error("Selected remote server is disconnected");
		const handler = findMockHandler(args.command ?? "");
		if (!handler) throw new Error("Unknown controlled remote command");
		return handler(args.args ?? {});
	},
	get_kubeconfig_sources: kubeconfigSources,
	set_kubeconfig_env_var: (args) => {
		kubeconfigSource.kubeconfigEnvVar = args.envVar?.trim() || "KUBECONFIG";
		return kubeconfigSources();
	},
	set_show_kubeconfig_source_labels: (args) => {
		kubeconfigSource.showSourceLabels = Boolean(args.show);
		return kubeconfigSources();
	},
	pick_kubeconfig_paths: () => {
		const path = selectedDialogPath();
		if (path && !kubeconfigSource.paths.some((entry) => entry.path === path)) kubeconfigSource.paths = [...kubeconfigSource.paths, { path }];
		return kubeconfigSources();
	},
	save_workspace_export_json: (args) => saveWorkspaceMock(String(args.content)),
	pick_workspace_import_json: selectedWorkspaceFile,
	add_kubeconfig_paths: (args) => addKubeconfigPathsMock(kubeconfigSource, args.paths),
	remove_kubeconfig_path: (args) => {
        kubeconfigSource.paths = kubeconfigSource.paths.filter((entry) => entry.path !== args.path);
        return kubeconfigSources();
    },
	reorder_kubeconfig_paths: (args) => reorderKubeconfigPathsMock(kubeconfigSource, args.paths),
	list_kube_contexts: () => shellRead("contexts", mockContexts),
	list_namespaces: () => shellRead("namespaces", namespaces),
	list_resource_kinds: () => shellRead("kinds", customResourceKinds()),
	list_present_custom_resource_kinds: (args) =>
		presentCustomResourceKinds(args?.namespaces),
	list_resources: (args) => filterResources(args?.kind, args?.namespace ?? undefined, args?.clusterContext),
	list_deployment_revisions: () => deploymentRevisions,
	...operationMockHandlers,
	list_dynamic_resources: (args) => filterResources(args?.resourceKind?.kind, args?.namespace ?? undefined, args?.clusterContext),
	list_resource_scope: (args) => {
        shellRead("resources", []);
        if (failedWorkspaceContext && args.clusterContext === failedWorkspaceContext) throw new Error("Context unavailable");
        return {
		resources: shellWithoutRemoved(listScope(args?.requests, args?.clusterContext)),
		completedRequests: args?.requests ?? [],
		// Read with the rows it describes, so a held read keeps its version.
		resourceVersions: (args?.requests ?? []).map((request) => ({ request, resourceVersion: mockListVersion() })),
		failures: [],
	}; },
	get_resource_yaml: (args) => builtInYamlFor(args),
	get_resource_details: (args) => withDetailStatusFixture(detailsFor(args)),
	get_dynamic_resource_details: (args) => withDetailStatusFixture(detailsFor(args)),
	prepare_yaml_apply: (args) => coverageYamlPreview(args?.request?.forceConflicts, applyPreview(args)),
	apply_yaml: unavailable("YAML apply is unavailable in browser mock mode."),
	lint_kubernetes_yaml: (args) => lintMockYaml(args.request),
	list_resource_events: (args) => eventsFor(args),
	list_resource_topology: (args) => coverageTopology(topologyFor(args)),
	refresh_resource_cache: () => ({ clearedEntries: 1 }),
	start_resource_watch: (args) => registerMockWatch({ channel: args.channel, context: args.clusterContext ?? "mock-dev", keys: args.keys }),
	start_resource_event_watch: (args) => registerMockWatch({ channel: args.channel, context: args.clusterContext ?? "mock-dev", kind: args.kind, name: args.name, namespace: args.namespace }),
	start_pod_log_stream: (args) => startLogStream(args?.channel, false, args?.request?.batchLines === true, args?.request),
	start_aggregated_log_stream: (args) => startLogStream(args?.channel, true, args?.request?.batchLines === true),
	stop_stream: (args) => stopMockWatch(args.id ?? ""),
    list_port_forwards: () => coverageSessions(listMockForwards()),
    list_pod_exec_sessions: () => coverageSessions(listMockExecs()),
	get_app_usage_metrics: () => usage(),
	list_resource_metrics: () => metrics(),
	...argoMockHandlers(genericYaml),
	reveal_secret_data_value: () => "c2VjcmV0",
	list_helm_releases: () => coverageHelm({ releases: helmReleases, warnings: [] } satisfies HelmReleaseList),
	get_helm_release_details: (args) => helmDetails(args),
	get_helm_release_reconciliation: (args) => helmReconciliation(args),
	list_rbac_inspection: (args) => coverageRbac(rbacInspectionMock(args?.clusterContext)),
	review_rbac_access: (args) => rbacReviewMock(args),
	list_incident_cockpit: (args) => coverageIncidents(incidents(args?.clusterContext)),
	set_backend_diagnostics_enabled: () => true,
	get_backend_diagnostics: () => [] satisfies BackendDiagnosticEvent[],
	get_backend_cache_diagnostics: () => [],
	clear_backend_diagnostics: () => undefined,
	cancel_backend_requests: (args) => ({ cancelled: mockTiming.cancelScope(args.cancelScope ?? "") }),
	cancel_workspace_requests: () => ({
		cancelledRequests: mockTiming.cancelAll(),
		cancelledLoads: 0,
		clientGeneration: 1,
	}),
} satisfies Record<string, MockHandler>;

function findMockHandler(command: string): MockHandler | undefined {
	return Object.entries(handlers).find(([registeredCommand]) => registeredCommand === command)?.[1];
}

function resourcesForCluster(cluster = "mock-dev"): ResourceSummary[] {
	const rows = cluster === "docker-desktop" ? dockerResources : resources;
	return rows.map((row) => ({ ...row, cluster }));
}


function filterResources(kindName?: string, namespace?: string, cluster?: string): ResourceSummary[] {
	return liveResources(resourcesForCluster(cluster)).filter((row) => (!kindName || row.kind === kindName) && (!namespace || row.namespace === namespace));
}

function listScope(requests: ResourceListRequest[] = [], cluster = "mock-dev"): ResourceSummary[] {
	const rows = liveResources(resourcesForCluster(cluster));
	if (requests.length === 0) return rows;
	const seen = new Set<string>();
	return requests.flatMap((request) => {
		const kindName = "resourceKind" in request && request.resourceKind ? request.resourceKind.kind : request.kind;
		return rows.filter((row) => (!kindName || row.kind === kindName) && (!request.namespace || row.namespace === request.namespace)).filter((row) => {
			const key = `${row.kind}:${row.namespace ?? ""}:${row.name}`;
			if (seen.has(key)) return false;
			seen.add(key);
			return true;
		});
	});
}

function resourceFromArgs(args: MockArgs): ResourceSummary {
	const rows = resourcesForCluster(args?.clusterContext);
	const dynamicKind = args?.resourceKind?.kind;
	const kind = args?.kind ?? dynamicKind;
	const match = rows.find((row) => row.kind === kind && row.name === args?.name && (args?.namespace === undefined || row.namespace === args.namespace));
	if (match) return match;
	if (kind === "Application" && args?.name !== undefined) {
		const summary = argoApplicationSummary(args.name);
		if (summary) return summary;
	}
	return rows[0];
}

function detailsFor(args: MockArgs): ResourceDetailsFull {
	const summary = resourceFromArgs(args);
	const yaml = yamlFor(args);
	return {
		summary,
		yaml: args.yamlEncoding === "kyaml" ? formatYamlDocument(yaml, "kyaml") : yaml,
		metadata: {},
		status: summary.name === evictedWorkerName ? evictedWorkerStatus : {},
	};
}

// Mirrors the Rust YAML command, which serves only built-in kinds.
function builtInYamlFor(args: MockArgs): string {
	const row = resourceFromArgs(args);
	if (row.dynamic) throw new Error(`unsupported resource kind: ${row.kind}`);
	let yaml = yamlFor(args);
	if (row.kind === "Deployment" && row.name === "payments-api") {
		const clean = args.yamlViewMode === "applyClean";
		const document = {
			apiVersion: "apps/v1", kind: row.kind,
			metadata: {
			    name: row.name, namespace: row.namespace, labels: { "kubecove.dev/mock": "true" },
			    ...(!clean && { uid: "mock-yaml-uid", resourceVersion: "7" }),
			},
			spec: {},
			...(!clean && { status: { replicas: 2 } }),
		};
		yaml = stringify(document);
	}
	return args?.yamlEncoding === "kyaml" ? formatYamlDocument(yaml, "kyaml") : yaml;
}

function yamlFor(args: MockArgs): string {
	const row = resourceFromArgs(args);
    if (row.kind === "Secret") return stringify({ apiVersion: "v1", kind: "Secret", metadata: { name: row.name, namespace: row.namespace }, data: { token: "REDACTED" } });
    if (row.namespace === "payments" && row.kind === "Pod" && row.name.startsWith("payments-api-")) return stringify({ apiVersion: "v1", kind: "Pod", metadata: { name: row.name, namespace: row.namespace, labels: { app: "payments-api" } }, spec: { containers: [{ name: "app", image: "example/payments-api", ports: [{ containerPort: 8080 }] }, { name: "sidecar", image: "example/sidecar" }] } });
    if (row.namespace === "payments" && row.kind === "Service" && row.name === "payments-api") return stringify({ apiVersion: "v1", kind: "Service", metadata: { name: row.name, namespace: row.namespace }, spec: { selector: { app: "payments-api" }, ports: [{ name: "http", port: 80, targetPort: 8080, protocol: "TCP" }] } });
	const app = row.kind === "Application" ? argoApps.find((candidate) => candidate.name === row.name) : undefined;
	if (app) return argoApplicationYaml(app);
	if (row.name === evictedWorkerName) return `${genericYaml(row.kind, row.name, row.namespace, row.apiVersion)}${stringify({ status: evictedWorkerStatus })}`;
	return genericYaml(row.kind, row.name, row.namespace, row.apiVersion);
}


function genericYaml(kindName: string, name: string, namespace?: string | null, apiVersion = "v1"): string {
	const ns = namespace ? `  namespace: ${namespace}\n` : "";
	return `apiVersion: ${apiVersion}\nkind: ${kindName}\nmetadata:\n  name: ${name}\n${ns}  labels:\n    kubecove.dev/mock: "true"\nspec: {}\n`;
}

function applyPreview(args: MockArgs): YamlApplyPreview {
	const request = args?.request;
	return {
		target: { clusterContext: request?.clusterContext ?? "mock-dev", kind: request?.kind ?? "Deployment", name: request?.name ?? "payments-api", namespace: request?.namespace },
		currentYaml: request?.yaml ?? genericYaml("Deployment", "payments-api", "payments", "apps/v1"),
		dryRunYaml: `${request?.yaml ?? genericYaml("Deployment", "payments-api", "payments", "apps/v1")}# dry-run: browser mock\n`,
	};
}

function eventsFor(args: MockArgs): ResourceEventSummary[] {
    const scripted = liveEvents(args.kind, args.name);
    if (scripted) return scripted;
	const row = resourceFromArgs(args);
	if (row.name === evictedWorkerName) return [];
	return [
		{ eventType: "Warning", reason: "Unhealthy", message: `${row.kind}/${row.name} failed a readiness probe in mock data.`, count: row.health === "healthy" ? 0 : 3, lastSeen: "2m ago", lastSeenAt: now, source: "kubelet", namespace: row.namespace },
		{ eventType: "Normal", reason: "Pulled", message: "Container image already present on machine.", count: 5, lastSeen: "8m ago", lastSeenAt: now, source: "kubelet", namespace: row.namespace },
	];
}

function topologyFor(args: MockArgs): ResourceTopology {
	const cluster = args?.clusterContext ?? "mock-dev";
	const namespaces = new Set(args?.namespaces ?? []);
	const mode = args?.mode === "networkFlow" ? "networkFlow" : "ownership";
	return mode === "networkFlow"
		? networkTopology(cluster, namespaces, resourcesForCluster(cluster))
		: ownershipTopology(cluster, namespaces, resourcesForCluster(cluster));
}

function ownershipTopology(cluster: string, namespaces: ReadonlySet<string>, rows: ResourceSummary[]): ResourceTopology {
	const deploymentRows = rows.filter((row) => row.kind === "Deployment" && inNamespaceScope(row, namespaces));
	const replicaSets = deploymentRows.map((row) => replicaSetFor(row, cluster));
	const podRows = rows.filter((row) => row.kind === "Pod" && inNamespaceScope(row, namespaces));
	const standaloneRows = rows.filter((row) => isOwnershipStandalone(row) && inNamespaceScope(row, namespaces) && !isOwnershipNoise(row));
	const nodes = [
		...deploymentRows.map((row) => topologyNode(row, cluster)),
		...replicaSets,
		...podRows.map((row) => topologyNode(row, cluster)),
		...standaloneRows.map((row) => topologyNode(row, cluster)),
	];
	const edges = [
		...deploymentRows.map((row) => topologyEdge(topologyId(row, cluster), replicaSetId(row, cluster), "owns")),
		...podRows.flatMap((row) => {
			const owner = deploymentRows.find((deployment) => deployment.namespace === row.namespace && row.name.startsWith(`${deployment.name}-`));
			return owner ? [topologyEdge(replicaSetId(owner, cluster), topologyId(row, cluster), "owns")] : [];
		}),
		...rows
			.filter((row) => inNamespaceScope(row, namespaces))
			.flatMap((resource) =>
				ownershipDependencyHints
					.filter((hint) =>
						hint.resourceKind === resource.kind &&
						hint.resourceName === resource.name &&
						hint.resourceNamespace === resource.namespace,
					)
					.map((hint) => {
						const owner = rows.find(
							(ownerRow) =>
								ownerRow.kind === hint.ownerKind &&
								ownerRow.name === hint.ownerName &&
								ownerRow.namespace === hint.ownerNamespace &&
								inNamespaceScope(ownerRow, namespaces),
						);
						if (!owner) return null;
						return topologyEdge(
							topologyId(owner, cluster),
							topologyId(resource, cluster),
							"owns",
						);
					})
					.filter((edge): edge is NonNullable<typeof edge> => edge !== null),
		),
	].filter((edge) => nodes.some((node) => node.id === edge.source) && nodes.some((node) => node.id === edge.target));
	return { nodes, edges, warnings: ["Browser mock ownership map includes curated support-resource ownership."] };
}

function networkTopology(cluster: string, namespaces: ReadonlySet<string>, allRows: ResourceSummary[]): ResourceTopology {
	const rows = allRows.filter(
		(row) => ["Ingress", "Service", "Pod"].includes(row.kind) && inNamespaceScope(row, namespaces),
	);
	const services = rows.filter((row) => row.kind === "Service");
	const ingresses = rows.filter((row) => row.kind === "Ingress");
	const pods = rows.filter((row) => row.kind === "Pod");
	const endpointSlices = services.flatMap((service) => {
		const key = serviceNamespaceKey(service);
		const mappedPodNames = new Set(networkServicePodMap[key] ?? []);
		const matchingPods = pods.filter((pod) => pod.namespace === service.namespace && mappedPodNames.has(pod.name));
		return matchingPods.length === 0 ? [] : [endpointSliceNode(service.name, service.namespace ?? "", cluster, matchingPods.map((pod) => pod.name))];
	});
	const nodes = [
		...rows.map((row) => topologyNode(row, cluster, networkPortHints(row))),
		...endpointSlices,
	];
	const edges = [
		...networkIngressTargets.map((link) => {
				const ingress = ingresses.find((row) =>
					row.kind === "Ingress" &&
					row.namespace === link.namespace &&
					row.name === link.ingress,
				);
				const service = services.find((row) =>
					row.kind === "Service" &&
					row.namespace === link.namespace &&
					row.name === link.service,
				);
				if (!ingress || !service) return null;
				return topologyEdge(topologyId(ingress, cluster), topologyId(service, cluster), "routesTo");
			})
			.filter((edge): edge is Exclude<ReturnType<typeof topologyEdge>, null> => edge !== null),
		...services.flatMap((service) => {
			const key = serviceNamespaceKey(service);
			const configuredPodNames = networkServicePodMap[key] ?? [];
			const configuredPods = pods.filter((pod) => configuredPodNames.includes(pod.name) && pod.namespace === service.namespace);
			if (configuredPods.length > 0) {
				const sliceSummary = endpointSliceSummary(service.name, service.namespace ?? "", cluster, configuredPods.map((pod) => pod.name));
				const fallbackEdges = [
					topologyEdge(topologyId(service, cluster), topologyId(sliceSummary, cluster), "targets"),
				];
				return fallbackEdges.concat(
					configuredPods.map((pod) => topologyEdge(topologyId(sliceSummary, cluster), topologyId(pod, cluster), "targets")),
				);
			}

			const selectorHint = networkServiceFallbackSelectors[key];
			const selectorMode = selectorHint?.split("=") ?? [];
			const selectorPodNames = selectorMode.length === 2
				? pods
					.filter((pod) => pod.namespace === service.namespace && pod.name.startsWith(`${selectorMode[1]}-`))
					.map((pod) => pod.name)
				: pods
					.filter((pod) => pod.name.startsWith(`${service.name}-`))
					.map((pod) => pod.name);

			const uniqueSelectorPods = [...new Set(selectorPodNames)];
			return uniqueSelectorPods.map((name) =>
				topologyEdge(topologyId(service, cluster), topologyId(resourceRef("Pod", name, service.namespace ?? ""), cluster), "selects"),
			);
		}),
	].filter((edge) => nodes.some((node) => node.id === edge.source) && nodes.some((node) => node.id === edge.target));
	return { nodes, edges, warnings: ["Browser mock network flow uses curated Service, Ingress, and EndpointSlice relationships."] };
}

function namespaceKeyOf(namespace: string | null, name: string): string {
	return `${namespace ?? ""}:${name}`;
}

function serviceNamespaceKey(service: Pick<ResourceSummary, "namespace" | "name">): string {
	return namespaceKeyOf(service.namespace, service.name);
}

function endpointSliceSummary(serviceName: string, namespace: string, cluster: string, pods: string[] = []): ResourceSummary {
	return {
		...res("EndpointSlice", `${serviceName}-mock`, namespace, "healthy", undefined, "Ready", 0, "platform", {
			apiVersion: "discovery.k8s.io/v1",
			cluster,
		}),
		age: "12d",
		status: "Ready",
		metrics: {
			kind: "EndpointSlice",
			cluster,
			name: `${serviceName}-mock`,
			namespace,
			cpuMillicores: 20,
			memoryBytes: 14_000_000,
			sampledAt: now,
			sourcePods: pods,
		},
	};
}

function isOwnershipStandalone(row: ResourceSummary): boolean {
	return ["Service", "Ingress", "ConfigMap", "Secret", "PersistentVolumeClaim", "PersistentVolume", "StorageClass"].includes(row.kind);
}

function isOwnershipNoise(row: ResourceSummary): boolean {
	return row.kind === "ConfigMap" && row.name === "kube-root-ca.crt";
}

function topologyNode(row: ResourceSummary, cluster: string, portHints: string[] = []) {
	return {
		id: topologyId(row, cluster),
		kind: row.kind,
		name: row.name,
		namespace: row.namespace,
		status: row.status,
		health: row.health,
		portHints,
		selectable: true,
		summary: { ...row, cluster },
	};
}

function replicaSetFor(row: ResourceSummary, cluster: string) {
	const name = row.name === "payments-api" ? "payments-api-7d9c9b7f8d" : `${row.name}-54b7f7bdbc`;
	const summary = res("ReplicaSet", name, row.namespace, row.health, row.ready, row.status, 0, row.argoApp ? "shop" : "platform", { apiVersion: "apps/v1", cluster });
	return topologyNode(summary, cluster);
}

function topologyId(row: Pick<ResourceSummary, "apiVersion" | "kind" | "namespace" | "name">, cluster: string): string {
	return `${cluster}:${row.apiVersion ?? "v1"}:${row.kind}:${row.namespace ?? ""}:${row.name}`;
}

function replicaSetId(row: ResourceSummary, cluster: string): string {
	const name = row.name === "payments-api" ? "payments-api-7d9c9b7f8d" : `${row.name}-54b7f7bdbc`;
	return topologyId({ apiVersion: "apps/v1", kind: "ReplicaSet", namespace: row.namespace, name }, cluster);
}

function endpointSliceNode(
	serviceName: string,
	namespace: string,
	cluster: string,
	pods: string[] = [],
): ReturnType<typeof topologyNode> {
	const summary = endpointSliceSummary(serviceName, namespace, cluster, pods);
	return { ...topologyNode(summary, cluster, networkPortHints(summary)), selectable: false };
}

function topologyEdge(source: string, target: string, relation: ResourceTopology["edges"][number]["relation"]) {
	return { id: `${source}->${target}`, source, target, relation };
}

function resourceRef(kindName: string, name: string, namespace: string): ResourceSummary {
	return resources.find((row) => row.kind === kindName && row.name === name && row.namespace === namespace) ?? res(kindName, name, namespace, "unknown");
}

function inNamespaceScope(row: Pick<ResourceSummary, "namespace"> | undefined, namespaces: ReadonlySet<string>): row is ResourceSummary {
	if (!row) return false;
	return namespaces.size === 0 || (row.namespace !== null && namespaces.has(row.namespace));
}

function networkPortHints(row: ResourceSummary): string[] {
	if (row.kind === "Ingress") return ["https:443->payments-api:80"];
	if (row.kind === "Service" && row.name === "payments-api") return ["http:80->8080", "metrics:9090->9090"];
	if (row.kind === "Service") return ["http:80->8080"];
	if (row.kind === "Pod") return ["app:8080", "metrics:9090"];
	return [];
}

function metrics(): ResourceMetricsSummary {
	const podMetrics = resources
		.filter((row) => row.kind === "Pod")
		.flatMap((row) => (row.metrics ? [row.metrics] : []));
	const workloads = resources.flatMap((row) => (row.metrics ? [row.metrics] : []));
	return { cluster: "mock-dev", availability: { status: "available" }, pods: podMetrics, nodes: [{ kind: "Node", cluster: "mock-dev", name: "dev-control-plane", namespace: null, cpuMillicores: 620, memoryBytes: 2_400_000_000, sampledAt: now, sourcePods: [] }], workloads, warnings: [] };
}

function usage(): AppUsageMetrics {
	return { cpuPercent: 3.8, memoryBytes: 388_000_000, processCount: 3, sampledAt: now, breakdown: [{ label: "KubeCove browser mock", description: "Bun tab with fake Tauri responses", cpuPercent: 3.8, memoryBytes: 388_000_000, processCount: 3, children: [] }] };
}

function helmReleaseFromArgs(args: MockArgs): HelmReleaseSummary {
	return helmReleases.find((release) => release.storageName === args?.storageName && release.namespace === args?.namespace) ?? helmReleases[0];
}

function helmDetails(args: MockArgs): HelmReleaseDetails {
	const summary = helmReleaseFromArgs(args);
	const manifestResources = summary.name === "grafana"
		? [{ apiVersion: "apps/v1", kind: "Deployment", name: "grafana", namespace: "monitoring" }, { apiVersion: "v1", kind: "Service", name: "grafana", namespace: "monitoring" }, { apiVersion: "networking.k8s.io/v1", kind: "Ingress", name: "grafana", namespace: "monitoring" }]
		: [{ apiVersion: "apps/v1", kind: "Deployment", name: "metrics-gateway", namespace: "platform" }, { apiVersion: "v1", kind: "Service", name: "metrics-gateway", namespace: "platform" }, { apiVersion: "v1", kind: "PersistentVolumeClaim", name: "metrics-gateway-cache", namespace: "platform" }, { apiVersion: "helm.toolkit.fluxcd.io/v2", kind: "HelmRelease", name: "metrics-gateway", namespace: "flux-system" }];
	return { summary, yaml: genericYaml("Secret", summary.storageName, summary.namespace), metadata: { labels: { owner: "helm", name: summary.name } }, valuesSummary: { hasValues: true, topLevelKeys: ["replicaCount", "image", "service", "resources"], valueCount: 8 }, manifestSummary: { resourceCount: manifestResources.length, resources: manifestResources, truncated: false }, release: { name: summary.name } };
}

function helmReconciliation(args: MockArgs): HelmReleaseReconciliation {
	const summary = helmReleaseFromArgs(args);
	const releaseResources = resources.filter((row) => row.helmRelease === summary.name);
	if (summary.name !== "metrics-gateway") {
		return { summary, totals: { tracked: releaseResources.length, unlabeledLive: 0, missing: 0, labelOnly: 0, unavailable: 0 }, resources: releaseResources.map((row) => ({ apiVersion: row.apiVersion, kind: row.kind, namespace: row.namespace ?? undefined, name: row.name, status: "tracked", statusMessage: "Tracked in mock Helm release", inManifest: true, explicitHelmLabel: true, liveResource: row })), warnings: [] };
	}
	const tracked: HelmReconciliationResource[] = resources.filter((row) => row.helmRelease === "metrics-gateway").map((row) => ({ apiVersion: row.apiVersion, kind: row.kind, namespace: row.namespace ?? undefined, name: row.name, status: "tracked", statusMessage: "Tracked in mock Helm release", inManifest: true, explicitHelmLabel: true, liveResource: row }));
	const ciliumSecret = resources.find((row) => row.kind === "Secret" && row.name === "sh.helm.release.v1.cilium.v2");
	return { summary, totals: { tracked: 4, unlabeledLive: 1, missing: 1, labelOnly: 0, unavailable: 0 }, resources: tracked.concat([{ apiVersion: "v1", kind: "ConfigMap", namespace: "platform", name: "metrics-gateway-dashboard", status: "missing", statusMessage: "Manifest resource is not present in mock live state.", inManifest: true, explicitHelmLabel: false }, { apiVersion: "v1", kind: "Secret", namespace: "kube-system", name: "sh.helm.release.v1.cilium.v2", status: "unlabeledLive", statusMessage: "Live Helm storage Secret is outside the selected release manifest.", inManifest: false, explicitHelmLabel: false, liveResource: ciliumSecret }]), warnings: [] };
}

function incidents(cluster = "mock-dev"): IncidentCockpitSummary {
	const items = resourcesForCluster(cluster).filter((row) => ["degraded", "attention", "restarted"].includes(row.health)).map((resource) => {
		const severity: IncidentSeverity = resource.health === "degraded" ? "degraded" : resource.health === "restarted" ? "restarted" : "attention";
		const warningEvents = eventsFor({ kind: resource.kind, name: resource.name, namespace: resource.namespace }).filter((event) => event.eventType === "Warning");
		return { resource: { ...resource, cluster }, severity, signals: [{ kind: resource.kind, label: resource.status ?? resource.health, message: `${resource.name} needs attention in mock data.`, source: "browser mock", lastSeenAt: now }], warningEventCount: warningEvents.length, latestSignalAt: now, latestWarningEvent: warningEvents[0] };
	});
	return { cluster, generatedAt: now, requestedScope: [], items, warnings: [] };
}

function startLogStream(
	channel: Pick<Channel<StreamMessage>, "onmessage"> | undefined,
	aggregate = false,
	batchLines = false,
	request = {},
): string {
	const streamId = `mock-logs-${Date.now()}`;
	const controlled = coverageLogs(request, streamId);
	if (controlled) { registerCoverageLogStream(streamId, (message) => send(channel, message)); for (const message of controlled) send(channel, message); return streamId; }
	send(channel, { type: "started", streamId, label: aggregate ? "Mock aggregate logs" : "Mock pod logs" });
	send(channel, { type: "status", streamId, status: "connected", message: aggregate ? "Streaming 2 pod containers" : "Mock log stream connected" });
	const sources = aggregate
		? [
				{ podName: "payments-api-7db7c9c9d4-8n2xq", container: "app" },
				{ podName: "payments-api-7db7c9c9d4-zl6pd", container: "app" },
			]
		: [undefined];
	for (const [index, line] of ["2026-06-29T10:00:00Z boot payments-api", "2026-06-29T10:00:02Z handled GET /healthz 200", "2026-06-29T10:00:04Z warning downstream retry budget at 82%"].entries()) {
		const source = sources[index % sources.length];
		send(channel, batchLines
			? { type: "logLines", streamId, lines: [line], source }
			: { type: "logLine", streamId, line, source });
	}
	return streamId;
}

function send(
	channel: Pick<Channel<StreamMessage>, "onmessage"> | undefined,
	message: StreamMessage,
): void {
	setTimeout(() => channel?.onmessage?.(message), 10);
}

function unavailable(message: string): MockHandler {
	return () => {
		throw new Error(message);
	};
}

function delay(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
}
