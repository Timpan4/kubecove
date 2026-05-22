import type { ResourceKindSelection, TopologyMode } from "./types";

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
	kubeContexts: () => ["kube-contexts"] as const,
	namespaces: (clusterContext: string) =>
		["kube-namespaces", clusterContext] as const,
	resourceKinds: (clusterContext: string) =>
		["kube-resource-kinds", clusterContext] as const,
	resources: (clusterContext: string, fetchKeys: ResourceFetchKey[]) =>
		["resources", clusterContext, ...resourceScopeParts(fetchKeys)] as const,
	resourceTopology: (
		clusterContext: string,
		namespaces: string[],
		mode: TopologyMode,
	) =>
		[
			"resource-topology",
			clusterContext,
			sortedNamespaces(namespaces),
			mode,
		] as const,
	resourceMetrics: (clusterContext: string, namespaces: string[]) =>
		["resource-metrics", clusterContext, sortedNamespaces(namespaces)] as const,
	argoDetect: (clusterContext: string) => ["argo-detect", clusterContext] as const,
	argoApps: (clusterContext: string) => ["argo-apps", clusterContext] as const,
	argoAppSets: (clusterContext: string) =>
		["argo-appsets", clusterContext] as const,
	argoAppProjects: (clusterContext: string) =>
		["argo-appprojects", clusterContext] as const,
	helmReleases: (clusterContext: string) =>
		["helm-releases", clusterContext] as const,
	rbacInspection: (clusterContext: string, namespaces: string[]) =>
		["rbac-inspection", clusterContext, sortedNamespaces(namespaces)] as const,
	helmReleaseDetails: (
		clusterContext: string,
		namespace: string,
		storageKind: string,
		storageName: string,
	) =>
		[
			"helm-release-details",
			clusterContext,
			namespace,
			storageKind,
			storageName,
		] as const,
	helmReleaseResources: (
		clusterContext: string,
		namespace: string,
		releaseName: string,
	) => ["helm-release-resources", clusterContext, namespace, releaseName] as const,
};
