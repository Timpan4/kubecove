import type { Channel, InvokeOptions } from "@tauri-apps/api/core";
import type { TauriClient } from "./tauri-runtime";
import {
	argoApps,
	dockerResources,
	fluxHelmKind,
	fluxKind,
	fluxResources,
	helmReleases,
	kinds,
	mockContexts,
	mockExecSessions,
	mockPortForwards,
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
import type {
	AppUsageMetrics,
	ArgoApplicationDetails,
	ArgoApplicationSetDetails,
	ArgoApplicationSetSummary,
	ArgoAppProjectDetails,
	ArgoAppProjectSummary,
	BackendDiagnosticEvent,
	DiscoveredResourceKind,
	FluxDetectionSummary,
	FluxResourceDetails,
	FluxResourceKind,
	HelmReleaseDetails,
	HelmReleaseReconciliation,
	HelmReconciliationResource,
	HelmReleaseSummary,
	IncidentCockpitSummary,
	IncidentSeverity,
	LiveSessionCleanupResult,
	ResourceDetailsFull,
	ResourceEventSummary,
	ResourceListRequest,
	ResourceMetricsSummary,
	ResourceSummary,
	ResourceTopology,
	RbacInspectionSummary,
	StreamMessage,
	YamlApplyPreview,
} from "./types";

type MockArgs = Record<string, unknown> | undefined;
type MockHandler = (args?: MockArgs, options?: InvokeOptions) => unknown | Promise<unknown>;

export function createDevMockTauriClient(): TauriClient {
	return {
		invoke: async <T>(cmd: string, args?: MockArgs, options?: InvokeOptions): Promise<T> => {
			await delay(35);
			const handler = handlers[cmd];
			if (!handler) throw new Error(`No browser dev mock for command: ${cmd}`);
			return (await handler(args, options)) as T;
		},
	};
}

const handlers: Record<string, MockHandler> = {
	get_kubeconfig_sources: () => source,
	set_kubeconfig_env_var: () => source,
	set_show_kubeconfig_source_labels: () => source,
	pick_kubeconfig_paths: () => source,
	add_kubeconfig_paths: () => source,
	remove_kubeconfig_path: () => source,
	reorder_kubeconfig_paths: () => source,
	list_kube_contexts: () => mockContexts,
	list_namespaces: () => namespaces,
	list_resource_kinds: () => kinds,
	list_resources: (args) => filterResources(args?.kind as string | undefined, args?.namespace as string | undefined, args?.clusterContext as string | undefined),
	list_dynamic_resources: (args) => filterResources((args?.resourceKind as DiscoveredResourceKind | undefined)?.kind, args?.namespace as string | undefined, args?.clusterContext as string | undefined),
	list_resource_scope: (args) => listScope(args?.requests as ResourceListRequest[] | undefined, args?.clusterContext as string | undefined),
	get_resource_yaml: (args) => yamlFor(args),
	get_resource_details: (args) => detailsFor(args),
	get_dynamic_resource_details: (args) => detailsFor(args),
	prepare_yaml_apply: (args) => applyPreview(args),
	apply_yaml: unavailable("YAML apply is unavailable in browser mock mode."),
	lint_kubernetes_yaml: () => ({ diagnostics: [], notes: [] }),
	list_resource_events: (args) => eventsFor(args),
	list_resource_topology: (args) => topologyFor(args),
	start_resource_watch: (args) => startStream(args?.channel, "mock-watch", "Resource watch connected"),
	start_resource_event_watch: (args) => startStream(args?.channel, "mock-events", "Event watch connected"),
	start_pod_log_stream: (args) => startLogStream(args?.channel),
	stop_stream: () => true,
	start_pod_port_forward: unavailable("Port forwarding is unavailable in browser mock mode."),
	stop_port_forward: () => true,
	list_port_forwards: () => mockPortForwards,
	start_pod_exec_session: unavailable("Pod exec is unavailable in browser mock mode."),
	write_pod_exec_stdin: () => false,
	resize_pod_exec_terminal: () => false,
	stop_pod_exec_session: () => true,
	list_pod_exec_sessions: () => mockExecSessions,
	stop_live_sessions_outside_scope: () => ({ stoppedPortForwardIds: [], stoppedPodExecIds: [], stoppedPortForwards: 0, stoppedPodExecSessions: 0 }) satisfies LiveSessionCleanupResult,
	get_app_usage_metrics: () => usage(),
	list_resource_metrics: () => metrics(),
	detect_argocd: () => true,
	list_argocd_applications: () => argoApps,
	get_argocd_application_details: (args) => argoDetails(args),
	list_argocd_appsets: () => appSets(),
	list_argocd_appprojects: () => projects(),
	get_argocd_appset_details: (args) => appSetDetails(args),
	get_argocd_appproject_details: (args) => projectDetails(args),
	detect_flux: () => ({ detected: true, kinds: [fluxKind, fluxHelmKind], missingKinds: [] }) satisfies FluxDetectionSummary,
	list_flux_resources: (args) => fluxResources.filter((row) => row.resourceKind.kind === (args?.resourceKind as FluxResourceKind | undefined)?.kind),
	get_flux_resource_details: (args) => fluxDetails(args),
	list_helm_releases: () => helmReleases,
	get_helm_release_details: (args) => helmDetails(args),
	get_helm_release_reconciliation: (args) => helmReconciliation(args),
	list_rbac_inspection: () => rbac(),
	list_incident_cockpit: (args) => incidents(args?.clusterContext as string | undefined),
	set_backend_diagnostics_enabled: () => true,
	get_backend_diagnostics: () => [] satisfies BackendDiagnosticEvent[],
	clear_backend_diagnostics: () => undefined,
	cancel_backend_requests: () => ({ cancelled: 0 }),
};

function resourcesForCluster(cluster = "mock-dev"): ResourceSummary[] {
	const rows = cluster === "docker-desktop" ? dockerResources : resources;
	return rows.map((row) => ({ ...row, cluster }));
}

function filterResources(kindName?: string, namespace?: string, cluster?: string): ResourceSummary[] {
	return resourcesForCluster(cluster).filter((row) => (!kindName || row.kind === kindName) && (!namespace || row.namespace === namespace));
}

function listScope(requests: ResourceListRequest[] = [], cluster = "mock-dev"): ResourceSummary[] {
	const rows = resourcesForCluster(cluster);
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
	const rows = resourcesForCluster(args?.clusterContext as string | undefined);
	const match = rows.find((row) => row.kind === args?.kind && row.name === args?.name && (args?.namespace === undefined || row.namespace === args.namespace));
	return match ?? rows[0];
}

function detailsFor(args: MockArgs): ResourceDetailsFull {
	const summary = resourceFromArgs(args);
	return {
		summary,
		yaml: yamlFor(args),
		metadata: { labels: { "app.kubernetes.io/name": summary.name }, annotations: { "kubecove.dev/mock": "true" } },
		status: { phase: summary.status, ready: summary.ready, restarts: summary.restarts },
	};
}

function yamlFor(args: MockArgs): string {
	const row = resourceFromArgs(args);
	return genericYaml(row.kind, row.name, row.namespace, row.apiVersion);
}

function genericYaml(kindName: string, name: string, namespace?: string | null, apiVersion = "v1"): string {
	const ns = namespace ? `  namespace: ${namespace}\n` : "";
	return `apiVersion: ${apiVersion}\nkind: ${kindName}\nmetadata:\n  name: ${name}\n${ns}  labels:\n    kubecove.dev/mock: "true"\nspec: {}\n`;
}

function applyPreview(args: MockArgs): YamlApplyPreview {
	const request = args?.request as { clusterContext?: string; kind?: string; name?: string; namespace?: string | null; yaml?: string } | undefined;
	return {
		target: { clusterContext: request?.clusterContext ?? "mock-dev", kind: request?.kind ?? "Deployment", name: request?.name ?? "payments-api", namespace: request?.namespace },
		currentYaml: request?.yaml ?? genericYaml("Deployment", "payments-api", "payments", "apps/v1"),
		dryRunYaml: `${request?.yaml ?? genericYaml("Deployment", "payments-api", "payments", "apps/v1")}# dry-run: browser mock\n`,
	};
}

function eventsFor(args: MockArgs): ResourceEventSummary[] {
	const row = resourceFromArgs(args);
	return [
		{ eventType: "Warning", reason: "Unhealthy", message: `${row.kind}/${row.name} failed a readiness probe in mock data.`, count: row.health === "healthy" ? 0 : 3, lastSeen: "2m ago", lastSeenAt: now, source: "kubelet", namespace: row.namespace },
		{ eventType: "Normal", reason: "Pulled", message: "Container image already present on machine.", count: 5, lastSeen: "8m ago", lastSeenAt: now, source: "kubelet", namespace: row.namespace },
	];
}

function topologyFor(args: MockArgs): ResourceTopology {
	const cluster = (args?.clusterContext as string | undefined) ?? "mock-dev";
	const namespaces = new Set((args?.namespaces as string[] | undefined) ?? []);
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
	return { ...topologyNode(summary, cluster), selectable: false };
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
	const podMetrics = resources.filter((row) => row.kind === "Pod").map((row) => row.metrics).filter(Boolean) as ResourceMetricsSummary["pods"];
	return { cluster: "mock-dev", availability: { status: "available" }, pods: podMetrics, nodes: [{ kind: "Node", cluster: "mock-dev", name: "dev-control-plane", namespace: null, cpuMillicores: 620, memoryBytes: 2_400_000_000, sampledAt: now, sourcePods: [] }], workloads: resources.map((row) => row.metrics).filter(Boolean) as ResourceMetricsSummary["workloads"], warnings: [] };
}

function usage(): AppUsageMetrics {
	return { cpuPercent: 3.8, memoryBytes: 388_000_000, processCount: 3, sampledAt: now, breakdown: [{ label: "KubeCove browser mock", description: "Vite tab with fake Tauri responses", cpuPercent: 3.8, memoryBytes: 388_000_000, processCount: 3, children: [] }] };
}

function argoDetails(args: MockArgs): ArgoApplicationDetails {
	const summary = argoApps.find((app) => app.name === args?.name) ?? argoApps[0];
	return { summary, yaml: genericYaml("Application", summary.name, summary.namespace, "argoproj.io/v1alpha1"), metadata: { labels: { "argocd.argoproj.io/instance": summary.name } }, status: { sync: { status: summary.syncStatus }, health: { status: summary.healthStatus } } };
}

function appSets(): ArgoApplicationSetSummary[] {
	return [
		{ name: "shop-generators", cluster: "mock-dev", namespace: "argocd", age: "18d", createdAt: "2026-06-11T08:00:00Z", project: "default", status: "Ready", syncStatus: "Synced", healthStatus: "Healthy", destinationNamespace: "payments", destinationServer: "https://kubernetes.default.svc", sourceRepo: "https://github.com/example/shop", sourceRevision: "main" },
		{ name: "tenant-previews", cluster: "mock-dev", namespace: "argocd", age: "9d", createdAt: "2026-06-20T08:00:00Z", project: "platform", status: "Ready", syncStatus: "Synced", healthStatus: "Healthy", destinationNamespace: "todo", destinationServer: "https://kubernetes.default.svc", sourceRepo: "https://github.com/example/previews", sourceRevision: "main" },
	];
}

function appSetDetails(args: MockArgs): ArgoApplicationSetDetails {
	const summary = appSets().find((appSet) => appSet.name === args?.name) ?? appSets()[0];
	return { summary, yaml: genericYaml("ApplicationSet", summary.name, summary.namespace, "argoproj.io/v1alpha1"), metadata: { labels: { "kubecove.dev/mock": "true" } } };
}

function projects(): ArgoAppProjectSummary[] {
	return [
		{ name: "default", cluster: "mock-dev", namespace: "argocd", age: "42d", createdAt: "2026-05-18T08:00:00Z", description: "Mock default project", status: "Active" },
		{ name: "platform", cluster: "mock-dev", namespace: "argocd", age: "31d", createdAt: "2026-05-29T08:00:00Z", description: "Platform-owned observability and ingress apps", status: "Active" },
	];
}

function projectDetails(args: MockArgs): ArgoAppProjectDetails {
	const summary = projects().find((project) => project.name === args?.name) ?? projects()[0];
	return { summary, yaml: genericYaml("AppProject", summary.name, summary.namespace, "argoproj.io/v1alpha1"), metadata: { labels: { "kubecove.dev/mock": "true" } } };
}

function fluxDetails(args: MockArgs): FluxResourceDetails {
	const summary = fluxResources.find((resource) => resource.name === args?.name && resource.resourceKind.kind === (args?.resourceKind as FluxResourceKind | undefined)?.kind) ?? fluxResources[0];
	return { summary, yaml: genericYaml(summary.resourceKind.kind, summary.name, summary.namespace, summary.resourceKind.apiVersion), metadata: { labels: { "kustomize.toolkit.fluxcd.io/name": summary.name } }, status: { conditions: [{ type: "Ready", status: summary.readyStatus ?? "Unknown", message: summary.message }] } };
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

function rbac(): RbacInspectionSummary {
	return {
		cluster: "mock-dev",
		warnings: [],
		serviceAccounts: [
			{ cluster: "mock-dev", name: "payments-api", namespace: "payments", age: "18d", createdAt: "2026-06-11T08:00:00Z", automountToken: true, secretsCount: 1, imagePullSecretsCount: 0, risks: [{ level: "medium", label: "Mounted token", reason: "Service account token is mounted." }] },
			{ cluster: "mock-dev", name: "grafana", namespace: "monitoring", age: "31d", createdAt: "2026-05-29T08:00:00Z", automountToken: false, secretsCount: 0, imagePullSecretsCount: 1, risks: [] },
			{ cluster: "mock-dev", name: "argocd-application-controller", namespace: "argocd", age: "36d", createdAt: "2026-05-24T08:00:00Z", automountToken: true, secretsCount: 2, imagePullSecretsCount: 0, risks: [{ level: "high", label: "Broad app access", reason: "Mock controller can update application resources." }] },
		],
		roles: [
			{ cluster: "mock-dev", kind: "Role", name: "payments-reader", namespace: "payments", age: "18d", createdAt: "2026-06-11T08:00:00Z", rulesCount: 1, risks: [], rules: [{ verbs: ["get", "list", "watch"], apiGroups: [""], resources: ["pods", "services", "configmaps"], resourceNames: [], nonResourceUrls: [], risks: [] }] },
			{ cluster: "mock-dev", kind: "Role", name: "grafana-config", namespace: "monitoring", age: "31d", createdAt: "2026-05-29T08:00:00Z", rulesCount: 1, risks: [], rules: [{ verbs: ["get"], apiGroups: [""], resources: ["secrets", "configmaps"], resourceNames: ["grafana"], nonResourceUrls: [], risks: [] }] },
		],
		clusterRoles: [
			{ cluster: "mock-dev", kind: "ClusterRole", name: "argocd-app-controller", age: "36d", createdAt: "2026-05-24T08:00:00Z", rulesCount: 2, risks: [{ level: "high", label: "Can update apps", reason: "Can update Argo CD Application resources." }], rules: [{ verbs: ["get", "list", "watch", "update"], apiGroups: ["argoproj.io"], resources: ["applications"], resourceNames: [], nonResourceUrls: [], risks: [{ level: "high", label: "Write access", reason: "Can update GitOps application state." }] }] },
		],
		roleBindings: [
			{ cluster: "mock-dev", kind: "RoleBinding", name: "payments-api-reader", namespace: "payments", age: "18d", createdAt: "2026-06-11T08:00:00Z", roleRefKind: "Role", roleRefName: "payments-reader", subjects: [{ kind: "ServiceAccount", name: "payments-api", namespace: "payments" }], risks: [] },
			{ cluster: "mock-dev", kind: "RoleBinding", name: "grafana-config", namespace: "monitoring", age: "31d", createdAt: "2026-05-29T08:00:00Z", roleRefKind: "Role", roleRefName: "grafana-config", subjects: [{ kind: "ServiceAccount", name: "grafana", namespace: "monitoring" }], risks: [] },
		],
		clusterRoleBindings: [
			{ cluster: "mock-dev", kind: "ClusterRoleBinding", name: "argocd-application-controller", age: "36d", createdAt: "2026-05-24T08:00:00Z", roleRefKind: "ClusterRole", roleRefName: "argocd-app-controller", subjects: [{ kind: "ServiceAccount", name: "argocd-application-controller", namespace: "argocd" }], risks: [{ level: "high", label: "Cluster-wide GitOps", reason: "Controller binding reaches all namespaces in mock data." }] },
		],
		namespaceAccess: [
			{ cluster: "mock-dev", namespace: "payments", serviceAccounts: 1, roles: 1, roleBindings: 1, boundSubjects: [{ kind: "ServiceAccount", name: "payments-api", namespace: "payments" }], risks: [] },
			{ cluster: "mock-dev", namespace: "monitoring", serviceAccounts: 1, roles: 1, roleBindings: 1, boundSubjects: [{ kind: "ServiceAccount", name: "grafana", namespace: "monitoring" }], risks: [] },
			{ cluster: "mock-dev", namespace: "argocd", serviceAccounts: 1, roles: 0, roleBindings: 0, boundSubjects: [{ kind: "ServiceAccount", name: "argocd-application-controller", namespace: "argocd" }], risks: [{ level: "high", label: "ClusterRoleBinding", reason: "Bound to cluster-wide application-controller role." }] },
		],
	};
}

function incidents(cluster = "mock-dev"): IncidentCockpitSummary {
	const items = resourcesForCluster(cluster).filter((row) => ["degraded", "attention", "restarted"].includes(row.health)).map((resource) => {
		const severity: IncidentSeverity = resource.health === "degraded" ? "degraded" : resource.health === "restarted" ? "restarted" : "attention";
		return { resource: { ...resource, cluster }, severity, signals: [{ kind: resource.kind, label: resource.status ?? resource.health, message: `${resource.name} needs attention in mock data.`, source: "browser mock", lastSeenAt: now }], latestWarningEvent: eventsFor({ kind: resource.kind, name: resource.name, namespace: resource.namespace })[0] };
	});
	return { cluster, generatedAt: now, requestedScope: [], items, warnings: [] };
}

function startStream(channel: unknown, idPrefix: string, message: string): string {
	const streamId = `${idPrefix}-${Date.now()}`;
	send(channel, { type: "started", streamId, label: message });
	send(channel, { type: "status", streamId, status: "connected", message });
	return streamId;
}

function startLogStream(channel: unknown): string {
	const streamId = `mock-logs-${Date.now()}`;
	send(channel, { type: "started", streamId, label: "Mock pod logs" });
	send(channel, { type: "status", streamId, status: "connected", message: "Mock log stream connected" });
	for (const line of ["2026-06-29T10:00:00Z boot payments-api", "2026-06-29T10:00:02Z handled GET /healthz 200", "2026-06-29T10:00:04Z warning downstream retry budget at 82%"]) {
		send(channel, { type: "logLine", streamId, line });
	}
	return streamId;
}

function send(channel: unknown, message: StreamMessage): void {
	const target = channel as Pick<Channel<StreamMessage>, "onmessage"> | undefined;
	setTimeout(() => target?.onmessage?.(message), 10);
}

function unavailable(message: string): MockHandler {
	return () => {
		throw new Error(message);
	};
}

function delay(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
}
