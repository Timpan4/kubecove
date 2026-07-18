import { kubeconfigSourceKey } from "./settings";
import type {
	FluxResourceKind,
	ResourceKindSelection,
	ResourceSummary,
	TopologyMode,
	YamlEncoding,
	YamlViewMode,
} from "./types";

interface ResourceFetchKey {
	kind: ResourceKindSelection;
	namespace?: string;
}

function resourceKindKey(kind: ResourceKindSelection): string {
	return typeof kind === "string"
		? `typed:${kind}`
		: `dynamic:${kind.apiVersion}:${kind.plural}:${kind.kind}`;
}

function sortedNamespaces(namespaces: string[]): string {
	return [...new Set(namespaces)].sort((a, b) => a.localeCompare(b)).join(",");
}

function resourceScopeParts(fetchKeys: ResourceFetchKey[]): string[] {
	return fetchKeys
		.map((key) => `${resourceKindKey(key.kind)}:${key.namespace ?? ""}`)
		.sort((a, b) => a.localeCompare(b));
}

export const finiteKubernetesQueryRoots = new Set([
	"kube-namespaces",
	"kube-resource-kinds",
	"kube-present-custom-resource-kinds",
	"resources",
	"deployment-revisions",
	"resource-topology",
	"resource-metrics",
	"resource-details",
	"resource-yaml",
	"resource-events",
	"argo-detect",
	"argo-apps",
	"argo-appsets",
	"argo-appprojects",
	"argo-app-details",
	"argo-appset-details",
	"argo-appproject-details",
	"flux-detect",
	"flux-resources",
	"flux-resource-details",
	"helm-releases",
	"helm-release-details",
	"helm-release-reconciliation",
	"rbac-inspection",
	"incident-cockpit",
]);

export function isFiniteKubernetesQuery(queryKey: readonly unknown[]): boolean {
	return typeof queryKey[0] === "string" && finiteKubernetesQueryRoots.has(queryKey[0]);
}

export const queryKeys = {
	kubeContexts: (kubeconfigEnvVar?: string) =>
		["kube-contexts", kubeconfigSourceKey(kubeconfigEnvVar)] as const,
	namespaces: (clusterContext: string, kubeconfigEnvVar?: string) =>
		["kube-namespaces", kubeconfigSourceKey(kubeconfigEnvVar), clusterContext] as const,
	resourceKinds: (clusterContext: string, kubeconfigEnvVar?: string) =>
		["kube-resource-kinds", kubeconfigSourceKey(kubeconfigEnvVar), clusterContext] as const,
	presentCustomResourceKinds: (
		clusterContext: string,
		namespaces: string[],
		kubeconfigEnvVar?: string,
	) =>
		[
			"kube-present-custom-resource-kinds",
			kubeconfigSourceKey(kubeconfigEnvVar),
			clusterContext,
			sortedNamespaces(namespaces),
		] as const,
	resources: (
		clusterContext: string,
		fetchKeys: ResourceFetchKey[],
		kubeconfigEnvVar?: string,
	) =>
		[
			"resources",
			kubeconfigSourceKey(kubeconfigEnvVar),
			clusterContext,
			...resourceScopeParts(fetchKeys),
		] as const,
	deploymentRevisions: (
		resource: ResourceSummary,
		kubeconfigEnvVar?: string,
	) =>
		[
			"deployment-revisions",
			kubeconfigSourceKey(kubeconfigEnvVar),
			resource.cluster,
			resource.namespace ?? "",
			resource.name,
		] as const,
	resourceTopology: (
		clusterContext: string,
		namespaces: string[],
		mode: TopologyMode,
		kubeconfigEnvVar?: string,
	) =>
		[
			"resource-topology",
			kubeconfigSourceKey(kubeconfigEnvVar),
			clusterContext,
			sortedNamespaces(namespaces),
			mode,
		] as const,
	resourceMetrics: (
		clusterContext: string,
		namespaces: string[],
		kubeconfigEnvVar?: string,
	) =>
		[
			"resource-metrics",
			kubeconfigSourceKey(kubeconfigEnvVar),
			clusterContext,
			sortedNamespaces(namespaces),
		] as const,
	resourceDetails: (
		resource: ResourceSummary,
		dynamicResourceKindKey?: string | null,
		kubeconfigEnvVar?: string,
		yamlViewMode: YamlViewMode = "kubectl",
		yamlEncoding: YamlEncoding = "yaml",
	) =>
		[
			"resource-details",
			kubeconfigSourceKey(kubeconfigEnvVar),
			dynamicResourceKindKey ?? "",
			resource.cluster,
			resource.apiVersion ?? "",
			resource.kind,
			resource.namespace ?? "",
			resource.name,
			yamlViewMode,
			yamlEncoding,
		] as const,
	resourceYaml: (
		resource: ResourceSummary,
		dynamicResourceKindKey?: string | null,
		kubeconfigEnvVar?: string,
		yamlViewMode: YamlViewMode = "kubectl",
		yamlEncoding: YamlEncoding = "yaml",
	) =>
		[
			"resource-yaml",
			kubeconfigSourceKey(kubeconfigEnvVar),
			dynamicResourceKindKey ?? "",
			resource.cluster,
			resource.apiVersion ?? "",
			resource.kind,
			resource.namespace ?? "",
			resource.name,
			yamlViewMode,
			yamlEncoding,
		] as const,
	resourceEvents: (
		resource: ResourceSummary,
		kubeconfigEnvVar?: string,
	) =>
		[
			"resource-events",
			kubeconfigSourceKey(kubeconfigEnvVar),
			resource.cluster,
			resource.apiVersion ?? "",
			resource.kind,
			resource.namespace ?? "",
			resource.name,
		] as const,
	argoDetect: (clusterContext: string, kubeconfigEnvVar?: string) =>
		["argo-detect", kubeconfigSourceKey(kubeconfigEnvVar), clusterContext] as const,
	argoApps: (clusterContext: string, kubeconfigEnvVar?: string) =>
		["argo-apps", kubeconfigSourceKey(kubeconfigEnvVar), clusterContext] as const,
	argoAppSets: (clusterContext: string, kubeconfigEnvVar?: string) =>
		["argo-appsets", kubeconfigSourceKey(kubeconfigEnvVar), clusterContext] as const,
	argoAppProjects: (clusterContext: string, kubeconfigEnvVar?: string) =>
		["argo-appprojects", kubeconfigSourceKey(kubeconfigEnvVar), clusterContext] as const,
	argoAppDetails: (
		clusterContext: string,
		name: string,
		namespace?: string | null,
		kubeconfigEnvVar?: string,
		yamlViewMode: YamlViewMode = "kubectl",
		yamlEncoding: YamlEncoding = "yaml",
	) =>
		[
			"argo-app-details",
			kubeconfigSourceKey(kubeconfigEnvVar),
			clusterContext,
			namespace ?? "",
			name,
			yamlViewMode,
			yamlEncoding,
		] as const,
	argoAppSetDetails: (
		clusterContext: string,
		name: string,
		namespace?: string | null,
		kubeconfigEnvVar?: string,
		yamlViewMode: YamlViewMode = "kubectl",
		yamlEncoding: YamlEncoding = "yaml",
	) =>
		[
			"argo-appset-details",
			kubeconfigSourceKey(kubeconfigEnvVar),
			clusterContext,
			namespace ?? "",
			name,
			yamlViewMode,
			yamlEncoding,
		] as const,
	argoAppProjectDetails: (
		clusterContext: string,
		name: string,
		namespace?: string | null,
		kubeconfigEnvVar?: string,
		yamlViewMode: YamlViewMode = "kubectl",
		yamlEncoding: YamlEncoding = "yaml",
	) =>
		[
			"argo-appproject-details",
			kubeconfigSourceKey(kubeconfigEnvVar),
			clusterContext,
			namespace ?? "",
			name,
			yamlViewMode,
			yamlEncoding,
		] as const,
	fluxDetect: (clusterContext: string, kubeconfigEnvVar?: string) =>
		["flux-detect", kubeconfigSourceKey(kubeconfigEnvVar), clusterContext] as const,
	fluxResources: (
		clusterContext: string,
		resourceKind: FluxResourceKind,
		kubeconfigEnvVar?: string,
	) =>
		[
			"flux-resources",
			kubeconfigSourceKey(kubeconfigEnvVar),
			clusterContext,
			resourceKind.apiVersion,
			resourceKind.plural,
			resourceKind.kind,
		] as const,
	fluxResourceDetails: (
		clusterContext: string,
		resourceKind: FluxResourceKind,
		name: string,
		namespace?: string | null,
		kubeconfigEnvVar?: string,
		yamlViewMode: YamlViewMode = "kubectl",
		yamlEncoding: YamlEncoding = "yaml",
	) =>
		[
			"flux-resource-details",
			kubeconfigSourceKey(kubeconfigEnvVar),
			clusterContext,
			resourceKind.apiVersion,
			resourceKind.plural,
			resourceKind.kind,
			namespace ?? "",
			name,
			yamlViewMode,
			yamlEncoding,
		] as const,
	helmReleases: (clusterContext: string, kubeconfigEnvVar?: string) =>
		["helm-releases", kubeconfigSourceKey(kubeconfigEnvVar), clusterContext] as const,
	rbacInspection: (
		clusterContext: string,
		sourceOrLegacyNamespaces?: string | string[],
		legacyKubeconfigEnvVar?: string,
	) =>
		[
			"rbac-inspection",
			kubeconfigSourceKey(
				typeof sourceOrLegacyNamespaces === "string"
					? sourceOrLegacyNamespaces
					: legacyKubeconfigEnvVar,
			),
			clusterContext,
		] as const,
	incidentCockpit: (
		clusterContext: string,
		fetchKeys: ResourceFetchKey[],
		kubeconfigEnvVar?: string,
	) =>
		[
			"incident-cockpit",
			kubeconfigSourceKey(kubeconfigEnvVar),
			clusterContext,
			...resourceScopeParts(fetchKeys),
		] as const,
	portForwards: () => ["port-forwards"] as const,
	podExecSessions: () => ["pod-exec-sessions"] as const,
	appUsageMetrics: () => ["app-usage-metrics"] as const,
	backendDiagnostics: () => ["backend-diagnostics"] as const,
	helmReleaseDetails: (
		clusterContext: string,
		namespace: string,
		storageKind: string,
		storageName: string,
		kubeconfigEnvVar?: string,
		yamlViewMode: YamlViewMode = "kubectl",
		yamlEncoding: YamlEncoding = "yaml",
	) =>
		[
			"helm-release-details",
			kubeconfigSourceKey(kubeconfigEnvVar),
			clusterContext,
			namespace,
			storageKind,
			storageName,
			yamlViewMode,
			yamlEncoding,
		] as const,
	helmReleaseReconciliation: (
		clusterContext: string,
		namespace: string,
		storageKind: string,
		storageName: string,
		kubeconfigEnvVar?: string,
	) =>
		[
			"helm-release-reconciliation",
			kubeconfigSourceKey(kubeconfigEnvVar),
			clusterContext,
			namespace,
			storageKind,
			storageName,
		] as const,
};
