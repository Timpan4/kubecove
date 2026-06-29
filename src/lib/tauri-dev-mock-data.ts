import type {
	ArgoApplicationSummary,
	ClusterContext,
	DiscoveredResourceKind,
	FluxResourceKind,
	FluxResourceSummary,
	HelmReleaseSummary,
	KubeconfigSourcesSummary,
	NamespaceSummary,
	PodExecSessionSummary,
	PortForwardSessionSummary,
	ResourceSummary,
} from "./types";


export const now = "2026-06-29T10:00:00Z";
export const source: KubeconfigSourcesSummary = {
	kubeconfigEnvVar: "kubeconfigSource=browser-mock",
	paths: [],
	sourceKey: "browser-mock",
	sourceLabel: "Browser mock",
	showSourceLabels: true,
	warnings: [],
};

export const mockContexts: ClusterContext[] = [
	{ name: "mock-dev", isCurrent: true },
	{ name: "admin@solid-k8s", isCurrent: false },
	{ name: "docker-desktop", isCurrent: false },
];

export const namespaces: NamespaceSummary[] = [
	{ name: "default", age: "42d", createdAt: "2026-05-18T08:00:00Z" },
	{ name: "argocd", age: "36d", createdAt: "2026-05-24T08:00:00Z" },
	{ name: "cert-manager", age: "36d", createdAt: "2026-05-24T08:00:00Z" },
	{ name: "keycloak", age: "28d", createdAt: "2026-06-01T08:00:00Z" },
	{ name: "kube-system", age: "42d", createdAt: "2026-05-18T08:00:00Z" },
	{ name: "monitoring", age: "31d", createdAt: "2026-05-29T08:00:00Z" },
	{ name: "opensearch", age: "24d", createdAt: "2026-06-05T08:00:00Z" },
	{ name: "traefik", age: "34d", createdAt: "2026-05-26T08:00:00Z" },
	{ name: "todo", age: "5d", createdAt: "2026-06-24T08:00:00Z" },
	{ name: "jobs-lab", age: "5d", createdAt: "2026-06-24T08:00:00Z" },
	{ name: "payments", age: "18d", createdAt: "2026-06-11T08:00:00Z" },
	{ name: "platform", age: "31d", createdAt: "2026-05-29T08:00:00Z" },
];

export const kinds: DiscoveredResourceKind[] = [
	kind("apps", "v1", "Deployment", "deployments", true),
	kind("", "v1", "Pod", "pods", true),
	kind("", "v1", "Service", "services", true),
	kind("networking.k8s.io", "v1", "Ingress", "ingresses", true),
	kind("discovery.k8s.io", "v1", "EndpointSlice", "endpointslices", true),
	kind("", "v1", "ConfigMap", "configmaps", true),
	kind("", "v1", "Secret", "secrets", true),
	kind("", "v1", "PersistentVolumeClaim", "persistentvolumeclaims", true),
	kind("", "v1", "Node", "nodes", false),
	kind("", "v1", "PersistentVolume", "persistentvolumes", false),
	kind("storage.k8s.io", "v1", "StorageClass", "storageclasses", false),
	kind("argoproj.io", "v1alpha1", "Application", "applications", true),
	kind("argoproj.io", "v1alpha1", "ApplicationSet", "applicationsets", true),
	kind("kustomize.toolkit.fluxcd.io", "v1", "Kustomization", "kustomizations", true),
	kind("helm.toolkit.fluxcd.io", "v2", "HelmRelease", "helmreleases", true),
];

export const resources: ResourceSummary[] = [
	res("Deployment", "argocd-server", "argocd", "healthy", "1/1", "Synced", 0, "argocd"),
	res("Pod", "argocd-server-8c9869674-xj9p", "argocd", "healthy", "1/1", "Running", 0, "argocd"),
	res("Service", "argocd-server", "argocd", "healthy", undefined, "ClusterIP", 0, "argocd"),
	res("ConfigMap", "argocd-cm", "argocd", "healthy", undefined, "Active", 0, "argocd"),
	res("Secret", "argocd-secret", "argocd", "unknown", undefined, "Redacted", 0, "argocd"),
	res("ConfigMap", "kube-root-ca.crt", "argocd", "unknown", undefined, "Active", 0, "argocd"),
	res("Deployment", "cert-manager", "cert-manager", "healthy", "1/1", "Ready", 0, "platform"),
	res("Pod", "cert-manager-65b765f58f-2w8cs", "cert-manager", "healthy", "1/1", "Running", 0, "platform"),
	res("Service", "cert-manager", "cert-manager", "healthy", undefined, "ClusterIP", 0, "platform"),
	res("Secret", "cert-manager-webhook-ca", "cert-manager", "unknown", undefined, "Redacted", 0, "platform"),
	res("Deployment", "coredns", "kube-system", "healthy", "2/2", "Ready", 0, "platform"),
	res("Pod", "coredns-7859998f6-lf7hm", "kube-system", "healthy", "1/1", "Running", 0, "platform"),
	res("Service", "kube-dns", "kube-system", "healthy", undefined, "ClusterIP", 0, "platform"),
	res("ConfigMap", "coredns", "kube-system", "healthy", undefined, "Active", 0, "platform"),
	res("Secret", "sh.helm.release.v1.cilium.v2", "kube-system", "unknown", undefined, "Redacted", 0, "platform"),
	res("Deployment", "grafana", "monitoring", "healthy", "1/1", "Synced", 0, "monitoring"),
	res("Pod", "grafana-5d7864bb9c-4twqz", "monitoring", "healthy", "1/1", "Running", 0, "monitoring"),
	res("Service", "grafana", "monitoring", "healthy", undefined, "ClusterIP", 0, "monitoring"),
	res("Ingress", "grafana", "monitoring", "healthy", undefined, "Ready", 0, "monitoring", { apiVersion: "networking.k8s.io/v1" }),
	res("ConfigMap", "grafana", "monitoring", "healthy", undefined, "Active", 0, "monitoring"),
	res("Secret", "grafana", "monitoring", "unknown", undefined, "Redacted", 0, "monitoring"),
	res("Deployment", "opensearch-dashboards", "opensearch", "attention", "1/1", "Synced", 1, "logs"),
	res("Pod", "opensearch-dashboards-85b6c79f6d-f9x7m", "opensearch", "attention", "1/1", "Running", 1, "logs"),
	res("Service", "opensearch-dashboards", "opensearch", "healthy", undefined, "ClusterIP", 0, "logs"),
	res("Ingress", "opensearch-dashboards", "opensearch", "attention", undefined, "RetryBudget", 0, "logs", { apiVersion: "networking.k8s.io/v1" }),
	res("ConfigMap", "opensearch-dashboards-config", "opensearch", "healthy", undefined, "Active", 0, "logs"),
	res("Secret", "opensearch-admin-password", "opensearch", "unknown", undefined, "Redacted", 0, "logs"),
	res("Deployment", "traefik", "traefik", "healthy", "1/1", "Ready", 0, "platform"),
	res("Pod", "traefik-6d89b8d6f8-mk7r5", "traefik", "healthy", "1/1", "Running", 0, "platform"),
	res("Service", "traefik", "traefik", "healthy", undefined, "LoadBalancer", 0, "platform"),
	res("Secret", "traefik-gateway-tls", "traefik", "unknown", undefined, "Redacted", 0, "platform"),
	res("Deployment", "payments-api", "payments", "healthy", "3/3", "Synced", 0, "shop"),
	res("Pod", "payments-api-7d9c9b7f8d-k2r9p", "payments", "attention", "1/1", "Running", 2, "shop"),
	res("Pod", "payments-api-7d9c9b7f8d-x4m8v", "payments", "healthy", "1/1", "Running", 0, "shop"),
	res("Service", "payments-api", "payments", "healthy", undefined, "ClusterIP", 0, "shop"),
	res("Ingress", "payments-api", "payments", "degraded", undefined, "MissingBackend", 0, "shop", { apiVersion: "networking.k8s.io/v1" }),
	res("ConfigMap", "payments-api-config", "payments", "healthy", undefined, "Active", 0, "shop"),
	res("Secret", "payments-api-secrets", "payments", "unknown", undefined, "Redacted", 0, "shop"),
	res("Deployment", "checkout-worker", "payments", "restarted", "2/2", "Synced", 4, "shop"),
	res("Pod", "checkout-worker-6bfbd7d5dd-9qj2f", "payments", "restarted", "1/1", "Running", 4, "shop"),
	res("Deployment", "todo-web", "todo", "healthy", "1/1", "Synced", 0, "todo"),
	res("Pod", "todo-web-54b7f7bdbc-5g7dz", "todo", "healthy", "1/1", "Running", 0, "todo"),
	res("Service", "todo-web", "todo", "healthy", undefined, "ClusterIP", 0, "todo"),
	res("Ingress", "todo-web", "todo", "healthy", undefined, "Ready", 0, "todo", { apiVersion: "networking.k8s.io/v1" }),
	res("ConfigMap", "todo-web-content", "todo", "healthy", undefined, "Active", 0, "todo"),
	res("Secret", "todo-web-session", "todo", "unknown", undefined, "Redacted", 0, "todo"),
	res("ConfigMap", "kube-root-ca.crt", "todo", "unknown", undefined, "Active", 0, "todo"),
	res("ConfigMap", "kube-root-ca.crt", "jobs-lab", "unknown", undefined, "Active", 0, "jobs"),
	res("Deployment", "metrics-gateway", "platform", "healthy", "2/2", "Synced", 0, "platform"),
	res("Service", "metrics-gateway", "platform", "healthy", undefined, "ClusterIP", 0, "platform"),
	res("Pod", "metrics-gateway-9f6d4f7c9-7j2p4", "platform", "healthy", "1/1", "Running", 0, "platform"),
	res("PersistentVolumeClaim", "metrics-gateway-cache", "platform", "healthy", "Bound", "Bound", 0, "platform"),
	res("PersistentVolume", "pv-metrics-gateway-cache", null, "healthy", "Bound", "Bound", 0, "platform", { apiVersion: "v1", namespaced: false }),
	res("StorageClass", "local-path", null, "healthy", undefined, "Default", 0, "platform", { apiVersion: "storage.k8s.io/v1", namespaced: false }),
	res("Application", "shop", "argocd", "attention", undefined, "OutOfSync", 0, "shop", {
		apiVersion: "argoproj.io/v1alpha1",
		group: "argoproj.io",
		version: "v1alpha1",
		plural: "applications",
		dynamic: true,
	}),
	res("Application", "observability", "argocd", "healthy", undefined, "Synced", 0, "monitoring", {
		apiVersion: "argoproj.io/v1alpha1",
		group: "argoproj.io",
		version: "v1alpha1",
		plural: "applications",
		dynamic: true,
	}),
	res("Kustomization", "platform", "flux-system", "healthy", undefined, "Ready", 0, "platform", {
		apiVersion: "kustomize.toolkit.fluxcd.io/v1",
		group: "kustomize.toolkit.fluxcd.io",
		version: "v1",
		plural: "kustomizations",
		dynamic: true,
		gitOpsOwner: { provider: "flux", kind: "Kustomization", name: "platform", namespace: "flux-system", confidence: "inventory" },
	}),
	res("HelmRelease", "metrics-gateway", "flux-system", "healthy", undefined, "Ready", 0, "platform", {
		apiVersion: "helm.toolkit.fluxcd.io/v2",
		group: "helm.toolkit.fluxcd.io",
		version: "v2",
		plural: "helmreleases",
		dynamic: true,
		gitOpsOwner: { provider: "flux", kind: "HelmRelease", name: "metrics-gateway", namespace: "flux-system", confidence: "inventory" },
	}),
	res("Node", "dev-control-plane", null, "healthy", "Ready", "Ready"),
];

export const dockerResources: ResourceSummary[] = [
	res("Deployment", "registry", "default", "healthy", "1/1", "Ready", 0, "local", { cluster: "docker-desktop" }),
	res("Pod", "registry-6fdd9f7f5c-2qz6p", "default", "healthy", "1/1", "Running", 0, "local", { cluster: "docker-desktop" }),
	res("Service", "registry", "default", "healthy", undefined, "ClusterIP", 0, "local", { cluster: "docker-desktop" }),
	res("Deployment", "ingress-nginx-controller", "ingress-nginx", "attention", "1/1", "Ready", 2, "edge", { cluster: "docker-desktop" }),
	res("Pod", "ingress-nginx-controller-7bdbf967f9-dk4nc", "ingress-nginx", "attention", "1/1", "Running", 2, "edge", { cluster: "docker-desktop" }),
	res("Service", "ingress-nginx-controller", "ingress-nginx", "healthy", undefined, "LoadBalancer", 0, "edge", { cluster: "docker-desktop" }),
	res("ConfigMap", "ingress-nginx-controller", "ingress-nginx", "healthy", undefined, "Active", 0, "edge", { cluster: "docker-desktop" }),
	res("Secret", "registry-pull-secret", "default", "unknown", undefined, "Redacted", 0, "local", { cluster: "docker-desktop" }),
	res("Node", "docker-desktop", null, "healthy", "Ready", "Ready", 0, "local", { cluster: "docker-desktop" }),
];

export const ownershipDependencyHints: Array<{
	ownerKind: string;
	ownerName: string;
	ownerNamespace: string;
	resourceKind: string;
	resourceName: string;
	resourceNamespace: string;
}> = [
	{ ownerKind: "Deployment", ownerName: "argocd-server", ownerNamespace: "argocd", resourceKind: "ConfigMap", resourceName: "argocd-cm", resourceNamespace: "argocd" },
	{ ownerKind: "Deployment", ownerName: "argocd-server", ownerNamespace: "argocd", resourceKind: "Secret", resourceName: "argocd-secret", resourceNamespace: "argocd" },
	{ ownerKind: "Deployment", ownerName: "argocd-server", ownerNamespace: "argocd", resourceKind: "Service", resourceName: "argocd-server", resourceNamespace: "argocd" },
	{ ownerKind: "Deployment", ownerName: "cert-manager", ownerNamespace: "cert-manager", resourceKind: "Secret", resourceName: "cert-manager-webhook-ca", resourceNamespace: "cert-manager" },
	{ ownerKind: "Deployment", ownerName: "cert-manager", ownerNamespace: "cert-manager", resourceKind: "Service", resourceName: "cert-manager", resourceNamespace: "cert-manager" },
	{ ownerKind: "Deployment", ownerName: "coredns", ownerNamespace: "kube-system", resourceKind: "ConfigMap", resourceName: "kube-root-ca.crt", resourceNamespace: "kube-system" },
	{ ownerKind: "Deployment", ownerName: "coredns", ownerNamespace: "kube-system", resourceKind: "ConfigMap", resourceName: "coredns", resourceNamespace: "kube-system" },
	{ ownerKind: "Deployment", ownerName: "coredns", ownerNamespace: "kube-system", resourceKind: "Service", resourceName: "kube-dns", resourceNamespace: "kube-system" },
	{ ownerKind: "Deployment", ownerName: "grafana", ownerNamespace: "monitoring", resourceKind: "ConfigMap", resourceName: "grafana", resourceNamespace: "monitoring" },
	{ ownerKind: "Deployment", ownerName: "grafana", ownerNamespace: "monitoring", resourceKind: "Secret", resourceName: "grafana", resourceNamespace: "monitoring" },
	{ ownerKind: "Deployment", ownerName: "grafana", ownerNamespace: "monitoring", resourceKind: "Service", resourceName: "grafana", resourceNamespace: "monitoring" },
	{ ownerKind: "Deployment", ownerName: "grafana", ownerNamespace: "monitoring", resourceKind: "Ingress", resourceName: "grafana", resourceNamespace: "monitoring" },
	{ ownerKind: "Deployment", ownerName: "opensearch-dashboards", ownerNamespace: "opensearch", resourceKind: "ConfigMap", resourceName: "opensearch-dashboards-config", resourceNamespace: "opensearch" },
	{ ownerKind: "Deployment", ownerName: "opensearch-dashboards", ownerNamespace: "opensearch", resourceKind: "Secret", resourceName: "opensearch-admin-password", resourceNamespace: "opensearch" },
	{ ownerKind: "Deployment", ownerName: "opensearch-dashboards", ownerNamespace: "opensearch", resourceKind: "Service", resourceName: "opensearch-dashboards", resourceNamespace: "opensearch" },
	{ ownerKind: "Deployment", ownerName: "opensearch-dashboards", ownerNamespace: "opensearch", resourceKind: "Ingress", resourceName: "opensearch-dashboards", resourceNamespace: "opensearch" },
	{ ownerKind: "Deployment", ownerName: "traefik", ownerNamespace: "traefik", resourceKind: "Service", resourceName: "traefik", resourceNamespace: "traefik" },
	{ ownerKind: "Deployment", ownerName: "payments-api", ownerNamespace: "payments", resourceKind: "ConfigMap", resourceName: "payments-api-config", resourceNamespace: "payments" },
	{ ownerKind: "Deployment", ownerName: "payments-api", ownerNamespace: "payments", resourceKind: "Secret", resourceName: "payments-api-secrets", resourceNamespace: "payments" },
	{ ownerKind: "Deployment", ownerName: "payments-api", ownerNamespace: "payments", resourceKind: "Service", resourceName: "payments-api", resourceNamespace: "payments" },
	{ ownerKind: "Deployment", ownerName: "payments-api", ownerNamespace: "payments", resourceKind: "Ingress", resourceName: "payments-api", resourceNamespace: "payments" },
	{ ownerKind: "Deployment", ownerName: "todo-web", ownerNamespace: "todo", resourceKind: "ConfigMap", resourceName: "todo-web-content", resourceNamespace: "todo" },
	{ ownerKind: "Deployment", ownerName: "todo-web", ownerNamespace: "todo", resourceKind: "Secret", resourceName: "todo-web-session", resourceNamespace: "todo" },
	{ ownerKind: "Deployment", ownerName: "todo-web", ownerNamespace: "todo", resourceKind: "Service", resourceName: "todo-web", resourceNamespace: "todo" },
	{ ownerKind: "Deployment", ownerName: "todo-web", ownerNamespace: "todo", resourceKind: "Ingress", resourceName: "todo-web", resourceNamespace: "todo" },
	{ ownerKind: "Deployment", ownerName: "metrics-gateway", ownerNamespace: "platform", resourceKind: "Service", resourceName: "metrics-gateway", resourceNamespace: "platform" },
	{ ownerKind: "Deployment", ownerName: "metrics-gateway", ownerNamespace: "platform", resourceKind: "PersistentVolumeClaim", resourceName: "metrics-gateway-cache", resourceNamespace: "platform" },
	{ ownerKind: "Deployment", ownerName: "traefik", ownerNamespace: "traefik", resourceKind: "Secret", resourceName: "traefik-gateway-tls", resourceNamespace: "traefik" },
];

export const networkServicePodMap: Record<string, string[]> = {
	"argocd:argocd-server": ["argocd-server-8c9869674-xj9p"],
	"kube-system:kube-dns": ["coredns-7859998f6-lf7hm"],
	"monitoring:grafana": ["grafana-5d7864bb9c-4twqz"],
	"opensearch:opensearch-dashboards": ["opensearch-dashboards-85b6c79f6d-f9x7m"],
	"traefik:traefik": ["traefik-6d89b8d6f8-mk7r5"],
	"payments:payments-api": ["payments-api-7d9c9b7f8d-k2r9p", "payments-api-7d9c9b7f8d-x4m8v"],
	"todo:todo-web": ["todo-web-54b7f7bdbc-5g7dz"],
	"platform:metrics-gateway": ["metrics-gateway-9f6d4f7c9-7j2p4"],
};

export const networkServiceFallbackSelectors: Record<string, string> = {
	"argocd:argocd-server": "app.kubernetes.io/name=argocd-server",
	"kube-system:kube-dns": "k8s-app=kube-dns",
	"monitoring:grafana": "app.kubernetes.io/name=grafana",
	"opensearch:opensearch-dashboards": "app.kubernetes.io/name=opensearch-dashboards",
	"traefik:traefik": "app.kubernetes.io/name=traefik",
	"payments:payments-api": "app.kubernetes.io/name=payments-api",
	"todo:todo-web": "app.kubernetes.io/name=todo-web",
	"platform:metrics-gateway": "app.kubernetes.io/name=metrics-gateway",
};

export const networkIngressTargets: Array<{
	namespace: string;
	ingress: string;
	service: string;
}> = [
	{ namespace: "monitoring", ingress: "grafana", service: "grafana" },
	{ namespace: "opensearch", ingress: "opensearch-dashboards", service: "opensearch-dashboards" },
	{ namespace: "payments", ingress: "payments-api", service: "payments-api" },
	{ namespace: "todo", ingress: "todo-web", service: "todo-web" },
];

export const argoApps: ArgoApplicationSummary[] = [
	{
		name: "shop",
		cluster: "mock-dev",
		namespace: "argocd",
		project: "default",
		syncStatus: "OutOfSync",
		healthStatus: "Progressing",
		destinationNamespace: "payments",
		destinationServer: "https://kubernetes.default.svc",
		sourceRepo: "https://github.com/example/shop",
		sourceRevision: "main",
		sourceMode: "git",
		sourceCount: 1,
		resourceNamespaces: ["payments"],
		trackedResourceCount: 8,
		age: "18d",
		createdAt: "2026-06-11T08:00:00Z",
	},
	{
		name: "observability",
		cluster: "mock-dev",
		namespace: "argocd",
		project: "platform",
		syncStatus: "Synced",
		healthStatus: "Healthy",
		destinationNamespace: "monitoring",
		destinationServer: "https://kubernetes.default.svc",
		sourceRepo: "https://github.com/example/observability",
		sourceRevision: "release-2026.06",
		sourceMode: "multi",
		sourceCount: 2,
		resourceNamespaces: ["monitoring", "opensearch"],
		trackedResourceCount: 9,
		age: "31d",
		createdAt: "2026-05-29T08:00:00Z",
	},
];

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

export const mockPortForwards: PortForwardSessionSummary[] = [
	{
		id: "mock-pf-grafana",
		clusterContext: "mock-dev",
		namespace: "monitoring",
		targetKind: "Service",
		targetName: "grafana",
		podName: "grafana-5d7864bb9c-4twqz",
		remotePort: 80,
		resolvedPodName: "grafana-5d7864bb9c-4twqz",
		resolvedPodPort: 3000,
		localPort: 13000,
		localAddress: "127.0.0.1",
		localUrl: "http://127.0.0.1:13000",
		status: "listening",
		startedAt: now,
	},
	{
		id: "mock-pf-payments",
		clusterContext: "mock-dev",
		namespace: "payments",
		targetKind: "Pod",
		targetName: "payments-api-7d9c9b7f8d-k2r9p",
		podName: "payments-api-7d9c9b7f8d-k2r9p",
		remotePort: 8080,
		resolvedPodName: "payments-api-7d9c9b7f8d-k2r9p",
		resolvedPodPort: 8080,
		localPort: 18080,
		localAddress: "127.0.0.1",
		localUrl: "http://127.0.0.1:18080",
		status: "connected",
		startedAt: now,
	},
];

export const mockExecSessions: PodExecSessionSummary[] = [
	{
		id: "mock-exec-payments",
		clusterContext: "mock-dev",
		namespace: "payments",
		podName: "payments-api-7d9c9b7f8d-k2r9p",
		container: "api",
		command: ["/bin/sh"],
		stdin: true,
		tty: true,
		terminalCols: 120,
		terminalRows: 34,
		status: "running",
		startedAt: now,
	},
];

function kind(group: string, version: string, k: string, plural: string, namespaced: boolean): DiscoveredResourceKind {
	return { group, version, apiVersion: group ? `${group}/${version}` : version, kind: k, plural, namespaced };
}

export function res(kindName: string, name: string, namespace: string | null, health: ResourceSummary["health"], ready?: string, status?: string, restarts = 0, app = "platform", extra: Partial<ResourceSummary> = {}): ResourceSummary {
	return {
		kind: kindName,
		cluster: "mock-dev",
		name,
		namespace,
		age: namespace === "payments" ? "18d" : "12d",
		apiVersion: extra.apiVersion ?? (kindName === "Deployment" ? "apps/v1" : "v1"),
		namespaced: namespace !== null,
		health,
		status,
		ready,
		restarts,
		createdAt: "2026-06-11T08:00:00Z",
		argoApp: app === "shop" ? "shop" : undefined,
		helmRelease: app === "platform" ? "metrics-gateway" : undefined,
		gitOpsOwner: extra.gitOpsOwner ?? (app === "shop" ? { provider: "argo", kind: "Application", name: "shop", namespace: "argocd", confidence: "metadata" } : undefined),
		metrics: namespace ? { kind: kindName, cluster: "mock-dev", name, namespace, cpuMillicores: restarts > 0 ? 180 : 42, memoryBytes: restarts > 0 ? 220_000_000 : 96_000_000, sampledAt: now, sourcePods: [] } : undefined,
		...extra,
	};
}
