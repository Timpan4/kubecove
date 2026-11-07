// Argo CD, Flux and Helm fixtures for the browser mocks.
import { mockArgoHealthyAssessment, mockHealthAssessment, now } from "./tauri-dev-mock-data";
import type { ArgoApplicationSummary, FluxResourceKind, FluxResourceSummary, HelmReleaseSummary, ResourceSummary } from "./types";

const labArgoApplications = [
	["root-application", "e2e-root", "argocd"],
	["platform-argocd", "e2e-platform", "argocd"],
	["platform-cilium", "e2e-platform", "kube-system"],
	["platform-metrics", "e2e-platform", "platform"],
	["platform-storage", "e2e-platform", "local-path-storage"],
	["platform-ingress", "e2e-platform", "traefik"],
	["tenant-catalog", "e2e-tenants", "tenant-catalog"],
	["tenant-ledger", "e2e-tenants", "tenant-ledger"],
] as const;

export const argoApps: ArgoApplicationSummary[] = labArgoApplications.map(([name, project, destinationNamespace]) => ({
	name,
	cluster: "mock-dev",
	namespace: "argocd",
	project,
	syncStatus: name === "tenant-ledger" ? "OutOfSync" : "Synced",
	healthStatus: name === "tenant-ledger" ? "Degraded" : "Healthy",
	healthAssessment: name === "tenant-ledger" ? mockHealthAssessment("degraded", "Degraded", "argoHealth") : mockArgoHealthyAssessment,
	destinationNamespace,
	destinationServer: "https://kubernetes.default.svc",
	sourceRepo: name.startsWith("platform-") ? "pinned Helm or internal Git" : "git://git-server.e2e-system.svc.cluster.local:9418/fixtures.git",
	sourceRevision: "main",
	sourceMode: "git",
	sourceCount: 1,
	sources: [
		{
			repoUrl: name.startsWith("platform-")
				? "pinned Helm or internal Git"
				: "git://git-server.e2e-system.svc.cluster.local:9418/fixtures.git",
			targetRevision: "main",
			resolvedRevision: "8f4c2d1",
			path: `apps/${name}`,
			chart: null,
			reference: null,
		},
	],
	resourceNamespaces: [destinationNamespace],
	trackedResourceCount: name.startsWith("tenant-") ? 12 : 8,
	age: "12d",
	createdAt: "2026-06-17T08:00:00Z",
}));

interface ArgoManagedResources {
	[applicationName: string]: readonly Pick<
		ResourceSummary,
		"kind" | "name" | "namespace"
	>[];
}

export const argoManagedResourceRefs: ArgoManagedResources = {
	"root-application": [{ kind: "Application", name: "shop", namespace: "argocd" }],
	"platform-argocd": [
		{ kind: "Deployment", name: "argocd-server", namespace: "argocd" },
		{ kind: "Pod", name: "argocd-server-8c9869674-xj9p", namespace: "argocd" },
		{ kind: "Service", name: "argocd-server", namespace: "argocd" },
		{ kind: "ConfigMap", name: "argocd-cm", namespace: "argocd" },
		{ kind: "Secret", name: "argocd-secret", namespace: "argocd" },
	],
	"platform-cilium": [
		{ kind: "Deployment", name: "coredns", namespace: "kube-system" },
		{ kind: "Pod", name: "coredns-7859998f6-lf7hm", namespace: "kube-system" },
		{ kind: "Service", name: "kube-dns", namespace: "kube-system" },
		{ kind: "ConfigMap", name: "coredns", namespace: "kube-system" },
		{ kind: "Secret", name: "sh.helm.release.v1.cilium.v2", namespace: "kube-system" },
	],
	"platform-metrics": [
		{ kind: "Deployment", name: "metrics-gateway", namespace: "platform" },
		{ kind: "Service", name: "metrics-gateway", namespace: "platform" },
		{ kind: "Pod", name: "metrics-gateway-9f6d4f7c9-7j2p4", namespace: "platform" },
		{ kind: "PersistentVolumeClaim", name: "metrics-gateway-cache", namespace: "platform" },
	],
	"platform-storage": [
		{ kind: "PersistentVolume", name: "pv-metrics-gateway-cache", namespace: null },
		{ kind: "StorageClass", name: "local-path", namespace: null },
	],
	"platform-ingress": [
		{ kind: "Deployment", name: "traefik", namespace: "traefik" },
		{ kind: "Pod", name: "traefik-6d89b8d6f8-mk7r5", namespace: "traefik" },
		{ kind: "Service", name: "traefik", namespace: "traefik" },
		{ kind: "Secret", name: "traefik-gateway-tls", namespace: "traefik" },
	],
	"tenant-catalog": [
		{ kind: "Deployment", name: "catalog", namespace: "tenant-catalog" },
		{ kind: "HorizontalPodAutoscaler", name: "catalog", namespace: "tenant-catalog" },
	],
	"tenant-ledger": [
		{ kind: "StatefulSet", name: "ledger", namespace: "tenant-ledger" },
		{ kind: "PersistentVolumeClaim", name: "data-ledger-0", namespace: "tenant-ledger" },
	],
};

export const fluxKind: FluxResourceKind = {
	group: "kustomize.toolkit.fluxcd.io",
	version: "v1",
	apiVersion: "kustomize.toolkit.fluxcd.io/v1",
	kind: "Kustomization",
	plural: "kustomizations",
	namespaced: true,
	category: "Kustomize",
};

export const fluxHelmKind: FluxResourceKind = {
	group: "helm.toolkit.fluxcd.io",
	version: "v2",
	apiVersion: "helm.toolkit.fluxcd.io/v2",
	kind: "HelmRelease",
	plural: "helmreleases",
	namespaced: true,
	category: "Helm",
};

export const fluxResources: FluxResourceSummary[] = [
	{
		cluster: "mock-dev",
		name: "platform",
		namespace: "flux-system",
		age: "31d",
		createdAt: "2026-05-29T08:00:00Z",
		resourceKind: fluxKind,
		healthAssessment: mockHealthAssessment("healthy", "True"),
		readyStatus: "True",
		suspended: false,
		sourceKind: "GitRepository",
		sourceName: "platform",
		sourceNamespace: "flux-system",
		interval: "5m",
		lastAppliedRevision: "main@sha1:6f2d1c8",
		message: "Applied revision main@sha1:6f2d1c8",
		inventory: [{ id: "platform_metrics-gateway_apps_Deployment" }],
	},
	{
		cluster: "mock-dev",
		name: "metrics-gateway",
		namespace: "flux-system",
		age: "12d",
		createdAt: "2026-06-17T08:00:00Z",
		resourceKind: fluxHelmKind,
		healthAssessment: mockHealthAssessment("healthy", "True"),
		readyStatus: "True",
		suspended: false,
		sourceKind: "HelmRepository",
		sourceName: "platform-charts",
		sourceNamespace: "flux-system",
		interval: "10m",
		lastAppliedRevision: "metrics-gateway-1.4.2",
		message: "Release reconciliation succeeded",
		inventory: [{ id: "platform_metrics-gateway_apps_Deployment" }, { id: "platform_metrics-gateway__Service" }],
	},
];

export const helmReleases: HelmReleaseSummary[] = [
	{
		cluster: "mock-dev",
		name: "operations",
		namespace: "operations",
		age: "12d",
		updatedAt: now,
		createdAt: "2026-06-17T08:00:00Z",
		chart: "kubecove-operations-0.1.0",
		appVersion: "1.0.0",
		revision: 1,
		status: "deployed",
		storageKind: "Secret",
		storageName: "sh.helm.release.v1.operations.v1",
	},
	{
		cluster: "mock-dev",
		name: "metrics-gateway",
		namespace: "platform",
		age: "12d",
		updatedAt: now,
		createdAt: "2026-06-17T08:00:00Z",
		chart: "metrics-gateway-1.4.2",
		appVersion: "1.4.2",
		revision: 7,
		status: "deployed",
		storageKind: "Secret",
		storageName: "sh.helm.release.v1.metrics-gateway.v7",
	},
	{
		cluster: "mock-dev",
		name: "grafana",
		namespace: "monitoring",
		age: "31d",
		updatedAt: now,
		createdAt: "2026-05-29T08:00:00Z",
		chart: "grafana-8.5.1",
		appVersion: "11.6.0",
		revision: 14,
		status: "deployed",
		storageKind: "Secret",
		storageName: "sh.helm.release.v1.grafana.v14",
	},
];
