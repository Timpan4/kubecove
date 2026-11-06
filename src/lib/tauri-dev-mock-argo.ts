import type { ArgoConnectionProfile, ArgoServerEndpoint } from "./gitops-types";
import type { MockArgs, MockHandler } from "./tauri-dev-mock-contract";
import { argoApps, argoManagedResourceRefs, fluxHelmKind, fluxKind, fluxResources, kinds, now, resources, source } from "./tauri-dev-mock-data";
import type { ArgoApplicationDetails, ArgoApplicationSetDetails, ArgoApplicationSetSummary, ArgoAppProjectDetails, ArgoAppProjectSummary, ArgoManagedResource, DiscoveredResourceKind, FluxDetectionSummary, FluxResourceDetails, ResourceSummary } from "./types";

type YamlFactory = (kind: string, name: string, namespace?: string | null, apiVersion?: string) => string;
export function customResourceKinds(): DiscoveredResourceKind[] {
	return kinds.filter((kind) =>
		[
			"argoproj.io",
			"kustomize.toolkit.fluxcd.io",
			"helm.toolkit.fluxcd.io",
            "testing.example.io",
		].includes(kind.group),
	);
}

export function presentCustomResourceKinds(namespaces: string[] = []): DiscoveredResourceKind[] {
	const namespaceSet = new Set(namespaces);
	return customResourceKinds().filter((kind) =>
		resources.some(
			(row) =>
				row.kind === kind.kind &&
				row.dynamic === true &&
				(namespaceSet.size === 0 || (row.namespace !== null && namespaceSet.has(row.namespace))),
		),
	);
}
let argoOutcomeComplete = false;
export type ArgoMockScript = "base" | "discovery-error" | "target-mismatch" | "snapshot-error" | "operation-error" | "lazy" | "full-diff";
let argoScript: ArgoMockScript = "base";
export function configureArgoMock(script: ArgoMockScript) { argoScript = script; }
export type ArgoInventoryMode = "base" | "absent" | "empty" | "partial" | "unavailable";
let argoInventoryMode: ArgoInventoryMode = "base";
export function configureArgoInventory(mode: ArgoInventoryMode) { argoInventoryMode = mode; }
export function completeMockArgoOperation() { argoOutcomeComplete = true; }
/** Mirrors the Rust endpoint model, which fills an absent tunnel TLS name when a profile is read. */
function normalizedArgoEndpoint(endpoint: ArgoServerEndpoint): ArgoServerEndpoint {
	if (endpoint.kind !== "serviceTunnel" || endpoint.tlsServerName) return endpoint;
	return { ...endpoint, tlsServerName: `${endpoint.serviceName}.${endpoint.namespace}.svc` };
}
const argoConnections = new Map<string, ArgoConnectionProfile>();

export function argoMockHandlers(genericYaml: YamlFactory) {
return {
	detect_argocd: () => {
        if (argoInventoryMode === "unavailable") throw new Error("Fixture provider detection unavailable");
        return argoInventoryMode !== "absent";
    },
	discover_argo_servers: () => {
        if (argoScript === "discovery-error") throw new Error("Fixture discovery unavailable");
        return [
		{ id: "service:argocd:argocd-server:443", name: "argocd-server", namespace: "argocd", url: null, transport: "serviceTunnel", endpoint: { kind: "serviceTunnel", namespace: "argocd", serviceName: "argocd-server", servicePort: 443, scheme: "https" }, unavailableReason: null, targetPod: "argocd-server-7d9f8b6c5-x2k4q", argoLabeled: true },
		{ id: "service:argocd:argocd-server:80", name: "argocd-server", namespace: "argocd", url: null, transport: "serviceTunnel", endpoint: { kind: "serviceTunnel", namespace: "argocd", serviceName: "argocd-server", servicePort: 80, scheme: "https" }, unavailableReason: null, targetPod: "argocd-server-7d9f8b6c5-x2k4q", argoLabeled: true },
		{ id: "service:argocd:external", name: "argocd-external", namespace: "argocd", url: null, transport: "serviceTunnel", endpoint: null, unavailableReason: "ExternalName Services cannot be port-forwarded", argoLabeled: false },
	]; },
	preview_argo_tunnel_target: (args) => {
		const endpoint = args?.endpoint?.kind === "serviceTunnel" ? args.endpoint : null;
		return {
			namespace: endpoint?.namespace ?? "argocd",
			serviceName: endpoint?.serviceName ?? "argocd-server",
			podName: "argocd-server-7d9f8b6c5-x2k4q",
			certificateSha256: endpoint?.scheme === "http" ? null : "3f".repeat(32),
		};
	},
	connect_argo_server: (args) => {
        if (argoScript === "target-mismatch") throw new Error("Fixture tunnel target changed");
		const serverUrl = args?.serverUrl ?? "https://argocd.example.test";
		const endpoint = normalizedArgoEndpoint(args?.endpoint ?? { kind: "externalHttps", url: serverUrl });
		const profile: ArgoConnectionProfile = { id: args?.id ?? "mock-profile", endpoint, url: serverUrl, clusterContext: args?.clusterContext ?? null, workspaceId: args?.workspaceId ?? null, kubeconfigSourceKey: args?.kubeconfigEnvVar ?? source.sourceKey, transport: "connected", rememberCredential: Boolean(args?.rememberCredential) };
		argoConnections.set(profile.id, profile);
		return { profile, connected: true, username: args?.username ?? "mock-user", unavailableReason: null, targetPod: "argocd-server-7d9f8b6c5-x2k4q", argoLabeled: true };
	},
	get_argo_connection_status: (args) => {
		const profile = args?.id ? argoConnections.get(args.id) ?? null : null;
		return { profile, connected: profile !== null, username: profile ? "mock-user" : null, unavailableReason: profile ? null : "Profile is not connected" };
	},
	disconnect_argo_server: (args) => {
		if (args?.id) argoConnections.delete(args.id);
	},
	forget_argo_credential: () => undefined,
	get_argo_application_inspector: (args) => {
        if (argoScript === "snapshot-error") throw new Error("Fixture snapshot unavailable");
		const app = argoApplication(args);
		const managedResources = argoManagedResources(args);
		return {
			application: {
				...args?.application,
				project: app.project,
				resourceVersion: "42",
				uid: `mock-${app.name}`,
			},
			status: {
				sync: { status: app.syncStatus ?? "Unknown", revision: "8f4c2d1" },
				health: { status: app.healthStatus ?? "Unknown" },
				reconciledAt: now,
			},
			history: [
				{ id: 18, revision: "8f4c2d1", revisions: [], deployedAt: now, initiatedBy: "mock-dev", source: null, sources: [] },
				{ id: 17, revision: "3d91a7b", revisions: [], deployedAt: "2026-06-28T09:30:00Z", initiatedBy: "automation", source: null, sources: [] },
			],
			resources: argoScript === "lazy" ? managedResources.map(resource => ({ ...resource, targetState: null, liveState: null })) : managedResources,
			comparisons: argoScript === "lazy" ? [] : managedResources.map((resource) =>
				argoComparison({ ...args, resource }),
			),
			conditions: [{ type: "ComparisonError", message: "Mock drift requires review before sync.", lastTransitionTime: now }],
			operationState: argoOutcomeComplete ? { phase: "Succeeded", startedAt: "2026-11-05T00:00:01Z", message: "Fixture operation finished" } : null,
			connected: args?.transport === "connected",
			transport: args?.transport === "connected" ? "connected" : "kubernetes",
			provenance:
				args?.transport === "connected"
					? "argocd-api"
					: "kubernetes-status-no-diff",
		};
	},
	get_argo_resource_comparison: (args) => argoComparison(args),
	preflight_argo_operation: (args) => {
        if (argoScript === "operation-error") throw new Error("Fixture preflight unavailable");
		const request = args?.request;
		const allowed = ["refresh", "hardRefresh", "sync", "retry"].includes(String(request?.action)) || (request?.action === "rollback" && request.transport === "connected");
		return { allowed, transport: request?.transport, action: request?.action, reason: allowed ? null : "operation unavailable in browser mock", sessionId: allowed ? "mock-session" : null, expiresAt: allowed ? Date.now() + 300_000 : null, reviewedRequest: allowed ? request : null };
	},
	run_argo_operation: () => ({ accepted: true, transport: "kubernetes", message: "Mock operation accepted", operation: null }),
	list_argocd_applications: () => {
        if (argoInventoryMode === "partial") throw new Error("Fixture Argo inventory unavailable");
        return argoInventoryMode === "empty" ? [] : argoApps;
    },
	get_argocd_application_details: (args) => argoDetails(args),
	list_argocd_appsets: () => appSets(),
	list_argocd_appprojects: () => projects(),
	get_argocd_appset_details: (args) => appSetDetails(args, genericYaml),
	get_argocd_appproject_details: (args) => projectDetails(args, genericYaml),
	detect_flux: () => {
        if (argoInventoryMode === "unavailable") throw new Error("Fixture provider detection unavailable");
        return { detected: argoInventoryMode !== "absent", kinds: argoInventoryMode === "absent" ? [] : [fluxKind, fluxHelmKind], missingKinds: [] } satisfies FluxDetectionSummary;
    },
	list_flux_resources: (args) => fluxResources.filter((row) => row.resourceKind.kind === args?.resourceKind?.kind),
	get_flux_resource_details: (args) => fluxDetails(args, genericYaml),
} satisfies Record<string, MockHandler>;
}

function argoApplication(args: MockArgs) {
	const application = args?.application;
	return argoApps.find((candidate) => candidate.name === application?.name) ?? argoApps[0];
}

function argoManagedResources(args: MockArgs): ArgoManagedResource[] {
	const app = argoApplication(args);
	const refs = argoManagedResourceRefs[app.name] ?? [];
	const rows = resources.filter(
		(row) =>
			row.name !== "kube-root-ca.crt" &&
			refs.some(
				(ref) =>
					ref.kind === row.kind &&
					ref.name === row.name &&
					ref.namespace === row.namespace,
			),
	);
	return rows.map((row) => {
		const progressing = row.kind === "ConfigMap";
		const degraded = row.kind === "Deployment";
		const prune = row.kind === "Secret";
		const apiVersion = row.apiVersion ?? "v1";
		const [group = "", version = "v1"] = apiVersion.includes("/")
			? apiVersion.split("/", 2)
			: ["", apiVersion];
		const targetState = prune ? undefined : argoManifest(row, true);
		const liveState = argoManifest(row, false);
		const resource: ArgoManagedResource = {
			group,
			version,
			kind: row.kind,
			namespace: row.namespace,
			name: row.name,
			status: row.kind === "Service" ? "Synced" : "OutOfSync",
			health: degraded ? "Degraded" : progressing ? "Progressing" : prune ? "Missing" : "Healthy",
			hook: false,
			requiresPruning: prune,
		};
		if (args?.transport === "connected") {
			resource.targetState = targetState;
			resource.liveState = liveState;
		}
		return resource;
	});
}

function argoComparison(args: MockArgs) {
	const requested = args?.resource;
	const resource = argoManagedResources({ ...args, transport: "connected" }).find(
		(candidate) =>
			candidate.group === requested?.group &&
			candidate.kind === requested?.kind &&
			candidate.namespace === requested?.namespace &&
			candidate.name === requested?.name,
	) ?? requested ?? {};
	if (args?.transport !== "connected") {
		return { resource, targetState: null, liveState: null, normalizedLiveState: null, predictedLiveState: null, modified: null, exact: false, provenance: "kubernetes-status-no-diff", availableActions: [] };
	}
	return {
		resource,
		targetState: resource.targetState ?? null,
		liveState: resource.liveState ?? null,
		normalizedLiveState: resource.liveState ?? null,
		predictedLiveState: resource.targetState ?? null,
		modified: resource.requiresPruning === true || JSON.stringify(resource.targetState) !== JSON.stringify(resource.liveState),
		exact: true,
		provenance: "argocd-managed-resource",
		availableActions: [],
	};
}

function argoManifest(row: ResourceSummary, desired: boolean) {
	return {
		apiVersion: row.apiVersion ?? "v1",
		kind: row.kind,
		metadata: {
			name: row.name,
			namespace: row.namespace,
			labels: { "app.kubernetes.io/managed-by": "argocd", "kubecove.dev/state": desired ? "desired" : "live" },
            annotations: argoScript === "full-diff" ? Object.fromEntries(Array.from({ length: 25 }, (_, index) => [`fixture-${index}`, desired ? "desired" : "live"])) : undefined,
		},
		spec: row.kind === "Deployment" ? { replicas: desired ? 3 : 2 } : undefined,
		data: row.kind === "ConfigMap" ? { release: desired ? "8f4c2d1" : "3d91a7b" } : undefined,
	};
}

function argoDetails(args: MockArgs): ArgoApplicationDetails {
	const summary = argoApps.find((app) => app.name === args?.name) ?? argoApps[0];
	return { summary, yaml: argoApplicationYaml(summary), metadata: { labels: { "argocd.argoproj.io/instance": summary.name } }, status: { sync: { status: summary.syncStatus }, health: { status: summary.healthStatus } } };
}

function appSets(): ArgoApplicationSetSummary[] {
	return [
		{ name: "shop-generators", cluster: "mock-dev", namespace: "argocd", age: "18d", createdAt: "2026-06-11T08:00:00Z", project: "default", status: "Ready", syncStatus: "Synced", healthStatus: "Healthy", destinationNamespace: "payments", destinationServer: "https://kubernetes.default.svc", sourceRepo: "https://github.com/example/shop", sourceRevision: "main" },
		{ name: "tenant-previews", cluster: "mock-dev", namespace: "argocd", age: "9d", createdAt: "2026-06-20T08:00:00Z", project: "platform", status: "Ready", syncStatus: "Synced", healthStatus: "Healthy", destinationNamespace: "todo", destinationServer: "https://kubernetes.default.svc", sourceRepo: "https://github.com/example/previews", sourceRevision: "main" },
	];
}

function appSetDetails(args: MockArgs, genericYaml: YamlFactory): ArgoApplicationSetDetails {
	const summary = appSets().find((appSet) => appSet.name === args?.name) ?? appSets()[0];
	return { summary, yaml: genericYaml("ApplicationSet", summary.name, summary.namespace, "argoproj.io/v1alpha1"), metadata: { labels: { "kubecove.dev/mock": "true" } } };
}

function projects(): ArgoAppProjectSummary[] {
	return [
		{ name: "default", cluster: "mock-dev", namespace: "argocd", age: "42d", createdAt: "2026-05-18T08:00:00Z", description: "Mock default project", status: "Active" },
		{ name: "platform", cluster: "mock-dev", namespace: "argocd", age: "31d", createdAt: "2026-05-29T08:00:00Z", description: "Platform-owned observability and ingress apps", status: "Active" },
	];
}

function projectDetails(args: MockArgs, genericYaml: YamlFactory): ArgoAppProjectDetails {
	const summary = projects().find((project) => project.name === args?.name) ?? projects()[0];
	return { summary, yaml: genericYaml("AppProject", summary.name, summary.namespace, "argoproj.io/v1alpha1"), metadata: { labels: { "kubecove.dev/mock": "true" } } };
}

function fluxDetails(args: MockArgs, genericYaml: YamlFactory): FluxResourceDetails {
	const summary = fluxResources.find((resource) => resource.name === args?.name && resource.resourceKind.kind === args?.resourceKind?.kind) ?? fluxResources[0];
	return { summary, yaml: genericYaml(summary.resourceKind.kind, summary.name, summary.namespace, summary.resourceKind.apiVersion), metadata: { labels: { "kustomize.toolkit.fluxcd.io/name": summary.name } }, status: { conditions: [{ type: "Ready", status: summary.readyStatus ?? "Unknown", message: summary.message }] } };
}

export function argoApplicationYaml(app: (typeof argoApps)[number]): string {
	return `apiVersion: argoproj.io/v1alpha1\nkind: Application\nmetadata:\n  name: ${app.name}\n  namespace: ${app.namespace ?? "argocd"}\nspec:\n  project: ${app.project ?? "default"}\n  source:\n    repoURL: ${app.sourceRepo ?? "https://github.com/example/platform-config.git"}\n    path: environments/mock-dev\n    targetRevision: ${app.sourceRevision ?? "main"}\n  destination:\n    server: ${app.destinationServer ?? "https://kubernetes.default.svc"}\n    namespace: ${app.destinationNamespace ?? "default"}\n  syncPolicy:\n    automated:\n      prune: true\n      selfHeal: true\n`;
}

export function argoApplicationSummary(name: string): ResourceSummary | undefined {
		const application = argoApps.find((candidate) => candidate.name === name);
	if (application) {
		const summary: ResourceSummary = {
			kind: "Application",
			cluster: application.cluster,
			name: application.name,
			namespace: application.namespace ?? null,
			age: application.age ?? "unknown",
			apiVersion: "argoproj.io/v1alpha1",
			group: "argoproj.io",
			version: "v1alpha1",
			plural: "applications",
			namespaced: true,
			dynamic: true,
			health: "healthy",
			createdAt: application.createdAt,
		};
		if (application.syncStatus) summary.status = application.syncStatus;
		return summary;
	}
}
