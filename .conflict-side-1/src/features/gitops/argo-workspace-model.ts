import type {
	ArgoApplicationHistory,
	ArgoApplicationSummary,
	ArgoManagedResource,
} from "@/lib/gitops-types";

export type ArgoPrototypePhase = "idle" | "refreshing" | "syncQueued" | "syncing";
export type ArgoDiffView = "changes" | "target" | "live" | "normalized" | "predicted";

export interface ArgoSyncSettings {
	revision: string;
	prune: boolean;
	dryRun: boolean;
	force: boolean;
}

export interface ArgoResourceCounts {
	total: number;
	outOfSync: number;
	degraded: number;
	progressing: number;
	missing: number;
	prune: number;
}

export interface ArgoPrototypeCondition {
	type: string;
	message: string;
	lastTransitionTime: string;
}

export interface ArgoPrototypeResource extends ArgoManagedResource {
	key: string;
	targetState: Record<string, unknown>;
	liveState: Record<string, unknown>;
	normalizedLiveState: Record<string, unknown>;
	predictedLiveState: Record<string, unknown>;
}

export interface ArgoPrototypeFixture {
	app: ArgoApplicationSummary;
	repository: string;
	path: string;
	destination: string;
	destinationNamespace: string;
	configuredRevision: string;
	resolvedRevision: string;
	reconciledAt: string;
	syncDefaults: ArgoSyncSettings;
	conditions: ArgoPrototypeCondition[];
	resources: ArgoPrototypeResource[];
	history: ArgoApplicationHistory[];
}

export const ARGO_SYNC_DEFAULTS: ArgoSyncSettings = {
	revision: "",
	prune: true,
	dryRun: false,
	force: false,
};

export function createArgoPrototypeFixture(app: ArgoApplicationSummary): ArgoPrototypeFixture {
	const namespace = app.destinationNamespace ?? app.resourceNamespaces[0] ?? "shop";
	const repository = app.sourceRepo ?? "https://github.com/example/platform-config.git";
	const configuredRevision = app.sourceRevision ?? "main";
	const deploymentTarget = deploymentManifest(namespace, app.name, "1.8.0", 4);
	const deploymentLive = deploymentManifest(namespace, app.name, "1.7.2", 3);
	const service = serviceManifest(namespace, app.name);
	const configMapTarget = configMapManifest(namespace, app.name, "payments-v2");
	const configMapLive = configMapManifest(namespace, app.name, "payments-v1");
	const job = jobManifest(namespace, `${app.name}-db-migrate`);
	const oldConfigMap = configMapManifest(namespace, `${app.name}-legacy`, "unused");

	return {
		app,
		repository,
		path: app.sources?.[0]?.path ?? "apps/shop",
		destination: app.destinationServer ?? "https://kubernetes.default.svc",
		destinationNamespace: namespace,
		configuredRevision,
		resolvedRevision: "8f4c2d1",
		reconciledAt: "2026-07-25T09:42:18Z",
		syncDefaults: { ...ARGO_SYNC_DEFAULTS },
		conditions: [
			{
				type: "ComparisonError",
				message: "Live Deployment differs from revision 8f4c2d1; two resources need reconciliation.",
				lastTransitionTime: "2026-07-25T09:42:18Z",
			},
		],
		resources: [
			prototypeResource("apps", "Deployment", namespace, app.name, "OutOfSync", "Degraded", deploymentTarget, deploymentLive),
			prototypeResource("", "Service", namespace, app.name, "Synced", "Healthy", service, service),
			prototypeResource("", "ConfigMap", namespace, `${app.name}-config`, "OutOfSync", "Progressing", configMapTarget, configMapLive),
			{
				...prototypeResource("batch", "Job", namespace, `${app.name}-db-migrate`, "Synced", "Healthy", job, job),
				hook: true,
			},
			{
				...prototypeResource("", "ConfigMap", namespace, `${app.name}-legacy`, "OutOfSync", "Missing", oldConfigMap, oldConfigMap),
				requiresPruning: true,
			},
		],
		history: [
			{
				id: 18,
				revision: "8f4c2d1",
				revisions: ["8f4c2d1"],
				deployedAt: "2026-07-24T16:31:07Z",
				initiatedBy: "automation",
				source: { repoURL: repository, path: "apps/shop", targetRevision: configuredRevision },
				sources: [],
			},
			{
				id: 17,
				revision: "61a9be4",
				revisions: ["61a9be4"],
				deployedAt: "2026-07-22T11:08:44Z",
				initiatedBy: "platform-team",
				source: { repoURL: repository, path: "apps/shop", targetRevision: configuredRevision },
				sources: [],
			},
		],
	};
}

export function argoResourceCounts(resources: ArgoManagedResource[]): ArgoResourceCounts {
	const normalized = resources.map((resource) => ({
		status: resource.status?.toLowerCase() ?? "",
		health: resource.health?.toLowerCase() ?? "",
		prune: resource.requiresPruning === true,
	}));
	return {
		total: resources.length,
		outOfSync: normalized.filter((resource) => resource.status === "outofsync").length,
		degraded: normalized.filter((resource) => resource.health === "degraded").length,
		progressing: normalized.filter((resource) => resource.health === "progressing").length,
		missing: normalized.filter((resource) => resource.health === "missing").length,
		prune: normalized.filter((resource) => resource.prune).length,
	};
}

export function argoPrototypeResources(
	fixture: ArgoPrototypeFixture,
	synced: boolean,
): ArgoPrototypeResource[] {
	if (!synced) return fixture.resources;
	return fixture.resources
		.filter((resource) => !resource.requiresPruning)
		.map((resource) => ({
			...resource,
			status: "Synced",
			health: "Healthy",
			liveState: resource.targetState,
			normalizedLiveState: resource.targetState,
			predictedLiveState: resource.targetState,
		}));
}

export function needsArgoSyncConfirmation(
	settings: ArgoSyncSettings,
	defaults: ArgoSyncSettings = ARGO_SYNC_DEFAULTS,
): boolean {
	return Boolean(
		settings.revision.trim() ||
		(settings.prune && !defaults.prune) ||
		(settings.force && !defaults.force),
	);
}

export function argoPhaseLabel(phase: ArgoPrototypePhase): string | null {
	if (phase === "refreshing") return "Refreshing";
	if (phase === "syncQueued") return "Sync queued";
	if (phase === "syncing") return "Syncing";
	return null;
}

function prototypeResource(
	group: string,
	kind: string,
	namespace: string,
	name: string,
	status: string,
	health: string,
	targetState: Record<string, unknown>,
	liveState: Record<string, unknown>,
): ArgoPrototypeResource {
	return {
		key: `${group}/${kind}/${namespace}/${name}`,
		group,
		version: kind === "Deployment" ? "v1" : "v1",
		kind,
		namespace,
		name,
		status,
		health,
		hook: false,
		requiresPruning: false,
		targetState,
		liveState,
		normalizedLiveState: liveState,
		predictedLiveState: targetState,
	};
}

function deploymentManifest(namespace: string, name: string, imageTag: string, replicas: number) {
	return {
		apiVersion: "apps/v1",
		kind: "Deployment",
		metadata: { name, namespace },
		spec: {
			replicas,
			selector: { matchLabels: { app: name } },
			template: {
				metadata: { labels: { app: name } },
				spec: { containers: [{ name: "app", image: `ghcr.io/example/${name}:${imageTag}` }] },
			},
		},
	};
}

function serviceManifest(namespace: string, name: string) {
	return {
		apiVersion: "v1",
		kind: "Service",
		metadata: { name, namespace },
		spec: { selector: { app: name }, ports: [{ port: 80, targetPort: 8080 }] },
	};
}

function configMapManifest(namespace: string, name: string, value: string) {
	return {
		apiVersion: "v1",
		kind: "ConfigMap",
		metadata: { name, namespace },
		data: { PAYMENT_PROVIDER: value, FEATURE_CHECKOUT_V2: "true" },
	};
}

function jobManifest(namespace: string, name: string) {
	return {
		apiVersion: "batch/v1",
		kind: "Job",
		metadata: { name, namespace, annotations: { "argocd.argoproj.io/hook": "PreSync" } },
		spec: { template: { spec: { restartPolicy: "Never", containers: [{ name: "migrate", image: "ghcr.io/example/migrate:8f4c2d1" }] } } },
	};
}
