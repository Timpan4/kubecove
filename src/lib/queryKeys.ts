import type {
	ResourceKindSelection,
	TopologyMode,
	YamlEncoding,
	YamlViewMode,
} from "./types";
import { kubeconfigSourceKey } from "./settings";

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

export const queryKeys = {
	kubeContexts: (kubeconfigEnvVar?: string) =>
		["kube-contexts", kubeconfigSourceKey(kubeconfigEnvVar)] as const,
	namespaces: (clusterContext: string, kubeconfigEnvVar?: string) =>
		["kube-namespaces", kubeconfigSourceKey(kubeconfigEnvVar), clusterContext] as const,
	resourceKinds: (clusterContext: string, kubeconfigEnvVar?: string) =>
		["kube-resource-kinds", kubeconfigSourceKey(kubeconfigEnvVar), clusterContext] as const,
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
	argoDetect: (clusterContext: string, kubeconfigEnvVar?: string) =>
		["argo-detect", kubeconfigSourceKey(kubeconfigEnvVar), clusterContext] as const,
	argoApps: (clusterContext: string, kubeconfigEnvVar?: string) =>
		["argo-apps", kubeconfigSourceKey(kubeconfigEnvVar), clusterContext] as const,
	argoAppSets: (clusterContext: string, kubeconfigEnvVar?: string) =>
		["argo-appsets", kubeconfigSourceKey(kubeconfigEnvVar), clusterContext] as const,
	argoAppProjects: (clusterContext: string, kubeconfigEnvVar?: string) =>
		["argo-appprojects", kubeconfigSourceKey(kubeconfigEnvVar), clusterContext] as const,
	helmReleases: (clusterContext: string, kubeconfigEnvVar?: string) =>
		["helm-releases", kubeconfigSourceKey(kubeconfigEnvVar), clusterContext] as const,
	rbacInspection: (
		clusterContext: string,
		namespaces: string[],
		kubeconfigEnvVar?: string,
	) =>
		[
			"rbac-inspection",
			kubeconfigSourceKey(kubeconfigEnvVar),
			clusterContext,
			sortedNamespaces(namespaces),
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
