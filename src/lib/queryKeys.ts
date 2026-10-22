import { matchQuery, type Query, type QueryClient } from "@tanstack/svelte-query";
import { finiteReadMeta } from "./finite-read-lifecycle";
import { type KindFetchKey, kindKey } from "./kind-identity";
import { kubeconfigSourceKey } from "./settings";
import type {
	FluxResourceKind,
	ResourceSummary,
	TopologyMode,
	YamlEncoding,
	YamlViewMode,
} from "./types";

// Query catalog: each TanStack read family is declared once with its key layout,
// ADR 0009 finite-read class, and retention class. Other modules build keys and
// select queries through this file and never read key positions.

type Retention = "default" | "large";

interface ReadFamily {
	root: string;
	finite: boolean;
	retention: Retention;
}

export const LARGE_QUERY_GC_TIME_MS = 300_000;

const finite = (root: string, retention: Retention = "default"): ReadFamily => ({
	root,
	finite: true,
	retention,
});
// Kubeconfig, session, connection-status, and diagnostics reads outlive workspace cancellation.
const nonFinite = (root: string): ReadFamily => ({ root, finite: false, retention: "default" });

const F = {
	kubeconfigSources: nonFinite("kubeconfig-sources"),
	kubeContexts: nonFinite("kube-contexts"),
	namespaces: finite("kube-namespaces"),
	resourceKinds: finite("kube-resource-kinds"),
	presentCustomResourceKinds: finite("kube-present-custom-resource-kinds"),
	resources: finite("resources", "large"),
	deploymentRevisions: finite("deployment-revisions"),
	resourceTopology: finite("resource-topology", "large"),
	resourceMetrics: finite("resource-metrics", "large"),
	resourceDetails: finite("resource-details"),
	resourceYaml: finite("resource-yaml"),
	resourceEvents: finite("resource-events"),
	argoConnectionStatuses: nonFinite("argo-connection-status"),
	argoServerDiscovery: finite("argo-server-discovery"),
	argoDetect: finite("argo-detect"),
	argoApps: finite("argo-apps"),
	argoAppSets: finite("argo-appsets"),
	argoAppProjects: finite("argo-appprojects"),
	argoAppDetails: finite("argo-app-details"),
	argoAppSetDetails: finite("argo-appset-details"),
	argoAppProjectDetails: finite("argo-appproject-details"),
	argoWorkspace: finite("argo-workspace"),
	fluxDetect: finite("flux-detect"),
	fluxResources: finite("flux-resources"),
	fluxResourceDetails: finite("flux-resource-details"),
	helmReleases: finite("helm-releases"),
	helmReleaseDetails: finite("helm-release-details"),
	helmReleaseReconciliation: finite("helm-release-reconciliation"),
	rbacInspection: finite("rbac-inspection"),
	incidentCockpit: finite("incident-cockpit"),
	portForwards: nonFinite("port-forwards"),
	podExecSessions: nonFinite("pod-exec-sessions"),
	appUsageMetrics: nonFinite("app-usage-metrics"),
	backendDiagnostics: nonFinite("backend-diagnostics"),
	backendCacheDiagnostics: nonFinite("backend-cache-diagnostics"),
	// Disabled placeholders for queries that have no target yet.
	idle: nonFinite("idle"),
};

const FAMILIES = new Map<unknown, ReadFamily>(Object.values(F).map((family) => [family.root, family]));
export const LARGE_QUERY_ROOTS = Object.values(F)
	.filter((family) => family.retention === "large")
	.map((family) => family.root);

type QueryKeyParts = readonly unknown[];

// Every cluster read starts with [root, source, context] so view matchers decode one layout.
function scoped(
	family: ReadFamily,
	source: string | undefined,
	context: string,
	...parts: unknown[]
): QueryKeyParts {
	return [family.root, kubeconfigSourceKey(source), context, ...parts];
}

// Resource reads follow with the resource identity at positions 3-6.
function resourceKey(family: ReadFamily, resource: ResourceSummary, source?: string, ...parts: unknown[]) {
	const { apiVersion, kind, namespace, name } = resource;
	return scoped(family, source, resource.cluster, apiVersion ?? "", kind, namespace ?? "", name, ...parts);
}

const contextRead = (family: ReadFamily) => (context: string, source?: string) =>
	scoped(family, source, context);

const yamlDocument =
	(family: ReadFamily) =>
	(
		context: string,
		name: string,
		namespace?: string | null,
		source?: string,
		yamlViewMode: YamlViewMode = "kubectl",
		yamlEncoding: YamlEncoding = "yaml",
	) =>
		scoped(family, source, context, namespace ?? "", name, yamlViewMode, yamlEncoding);

function sortedNamespaces(namespaces: string[]): string {
	return [...new Set(namespaces)].sort((a, b) => a.localeCompare(b)).join(",");
}

function resourceScopeParts(fetchKeys: KindFetchKey[]): string[] {
	return fetchKeys
		.map((key) => `${kindKey(key.kind)}:${key.namespace ?? ""}`)
		.sort((a, b) => a.localeCompare(b));
}

/** The representation choices that give one resource several details and YAML reads. */
export interface ResourceReadView {
	dynamicKindKey?: string | null;
	yamlViewMode?: YamlViewMode;
	yamlEncoding?: YamlEncoding;
	redactSecrets?: boolean;
}

function viewParts({
	dynamicKindKey,
	yamlViewMode = "kubectl",
	yamlEncoding = "yaml",
	redactSecrets = true,
}: ResourceReadView) {
	return [dynamicKindKey ?? "", yamlViewMode, yamlEncoding, redactSecrets];
}

type ArgoTransport = "connected" | "kubernetes";

export const queryKeys = {
	kubeconfigSources: () => [F.kubeconfigSources.root] as const,
	kubeContexts: (source?: string) => [F.kubeContexts.root, kubeconfigSourceKey(source)] as const,
	namespaces: contextRead(F.namespaces),
	resourceKinds: contextRead(F.resourceKinds),
	presentCustomResourceKinds: (context: string, namespaces: string[], source?: string) =>
		scoped(F.presentCustomResourceKinds, source, context, sortedNamespaces(namespaces)),
	resources: (context: string, fetchKeys: KindFetchKey[], source?: string) =>
		scoped(F.resources, source, context, ...resourceScopeParts(fetchKeys)),
	deploymentRevisions: (resource: ResourceSummary, source?: string) =>
		resourceKey(F.deploymentRevisions, resource, source),
	/** `customResourceKinds` refetches the map when the browsed custom kinds change. */
	resourceTopology: (
		context: string,
		namespaces: string[],
		mode: TopologyMode,
		source?: string,
		customResourceKinds = "",
	) => scoped(F.resourceTopology, source, context, sortedNamespaces(namespaces), mode, customResourceKinds),
	resourceMetrics: (context: string, namespaces: string[], source?: string) =>
		scoped(F.resourceMetrics, source, context, sortedNamespaces(namespaces)),
	resourceDetails: (resource: ResourceSummary, source?: string, view: ResourceReadView = {}) =>
		resourceKey(F.resourceDetails, resource, source, ...viewParts(view)),
	/** `refreshVersion` gives a watch-driven YAML refresh its own entry. */
	resourceYaml: (resource: ResourceSummary, source?: string, view: ResourceReadView = {}, refreshVersion = 0) =>
		resourceKey(F.resourceYaml, resource, source, ...viewParts(view), refreshVersion),
	/** Prefix of every events read for the resource, whichever UID it was matched by. */
	resourceEventsScope: (resource: ResourceSummary, source?: string) =>
		resourceKey(F.resourceEvents, resource, source),
	resourceEvents: (resource: ResourceSummary, source?: string, uid?: string) =>
		resourceKey(F.resourceEvents, resource, source, uid ?? ""),
	argoConnectionStatuses: (
		context: string,
		workspaceId: string,
		profileIds: readonly string[],
		source?: string,
	) => scoped(F.argoConnectionStatuses, source, context, workspaceId, profileIds),
	argoServerDiscovery: contextRead(F.argoServerDiscovery),
	argoDetect: contextRead(F.argoDetect),
	argoApps: contextRead(F.argoApps),
	argoAppSets: contextRead(F.argoAppSets),
	argoAppProjects: contextRead(F.argoAppProjects),
	argoAppDetails: yamlDocument(F.argoAppDetails),
	argoAppSetDetails: yamlDocument(F.argoAppSetDetails),
	argoAppProjectDetails: yamlDocument(F.argoAppProjectDetails),
	/** Prefix of every read for one workspace Application, whatever its UID or transport. */
	argoWorkspaceApplicationScope: (
		context: string,
		workspaceId: string,
		name: string,
		namespace: string | null | undefined,
		source?: string,
	) => scoped(F.argoWorkspace, source, context, workspaceId, namespace ?? "", name),
	argoWorkspaceApplication: (
		context: string,
		workspaceId: string,
		name: string,
		namespace: string | null | undefined,
		uid: string | null | undefined,
		redactSecrets: boolean,
		source?: string,
	) => [
		...queryKeys.argoWorkspaceApplicationScope(context, workspaceId, name, namespace, source),
		uid ?? "",
		redactSecrets,
	],
	argoWorkspaceInspector: (
		context: string,
		workspaceId: string,
		name: string,
		namespace: string | null | undefined,
		uid: string | null | undefined,
		redactSecrets: boolean,
		transport: ArgoTransport,
		connectionId?: string | null,
		source?: string,
	) => [
		...queryKeys.argoWorkspaceApplication(context, workspaceId, name, namespace, uid, redactSecrets, source),
		transport,
		transport === "kubernetes" ? "" : (connectionId ?? ""),
		"inspector",
	],
	argoWorkspaceSelectedResource: (
		context: string,
		workspaceId: string,
		name: string,
		namespace: string | null | undefined,
		uid: string | null | undefined,
		redactSecrets: boolean,
		transport: ArgoTransport,
		connectionId: string | null | undefined,
		group: string | null | undefined,
		kind: string | null | undefined,
		resourceNamespace: string | null | undefined,
		resourceName: string | null | undefined,
		source?: string,
	) => [
		...queryKeys.argoWorkspaceApplication(context, workspaceId, name, namespace, uid, redactSecrets, source),
		transport,
		transport === "kubernetes" ? "" : (connectionId ?? ""),
		"selected-resource",
		group ?? "",
		kind ?? "",
		resourceNamespace ?? "",
		resourceName ?? "",
	],
	fluxDetect: contextRead(F.fluxDetect),
	fluxResources: (context: string, kind: FluxResourceKind, source?: string) =>
		scoped(F.fluxResources, source, context, kind.apiVersion, kind.plural, kind.kind),
	fluxResourceDetails: (
		context: string,
		kind: FluxResourceKind,
		name: string,
		namespace?: string | null,
		source?: string,
		yamlViewMode: YamlViewMode = "kubectl",
		yamlEncoding: YamlEncoding = "yaml",
	) =>
		scoped(
			F.fluxResourceDetails,
			source,
			context,
			kind.apiVersion,
			kind.plural,
			kind.kind,
			namespace ?? "",
			name,
			yamlViewMode,
			yamlEncoding,
		),
	helmReleases: contextRead(F.helmReleases),
	helmReleaseDetails: (
		context: string,
		namespace: string,
		storageKind: string,
		storageName: string,
		source?: string,
		yamlViewMode: YamlViewMode = "kubectl",
		yamlEncoding: YamlEncoding = "yaml",
	) =>
		scoped(F.helmReleaseDetails, source, context, namespace, storageKind, storageName, yamlViewMode, yamlEncoding),
	helmReleaseReconciliation: (
		context: string,
		namespace: string,
		storageKind: string,
		storageName: string,
		source?: string,
	) => scoped(F.helmReleaseReconciliation, source, context, namespace, storageKind, storageName),
	rbacInspection: contextRead(F.rbacInspection),
	incidentCockpit: (context: string, fetchKeys: KindFetchKey[], source?: string) =>
		scoped(F.incidentCockpit, source, context, ...resourceScopeParts(fetchKeys)),
	portForwards: () => [F.portForwards.root] as const,
	podExecSessions: () => [F.podExecSessions.root] as const,
	appUsageMetrics: () => [F.appUsageMetrics.root] as const,
	backendDiagnostics: () => [F.backendDiagnostics.root] as const,
	backendCacheDiagnostics: () => [F.backendCacheDiagnostics.root] as const,
	/** A disabled query's key while it has no target; never fetched or cancelled. */
	idle: (label: string) => [F.idle.root, label] as const,
};

/** Applies each family's finite-read metadata and retention to every query under its root. */
export function configureQueryCatalog(queryClient: QueryClient): void {
	for (const family of FAMILIES.values()) {
		const defaults = queryClient.getQueryDefaults([family.root]);
		if (family.finite) defaults.meta = finiteReadMeta();
		if (family.retention === "large") defaults.gcTime = LARGE_QUERY_GC_TIME_MS;
		queryClient.setQueryDefaults([family.root], defaults);
	}
}

/** The declared family root for profiling, or "other" for keys outside the catalog. */
export function queryFamilyRoot(queryKey: readonly unknown[]): string {
	return FAMILIES.get(queryKey[0])?.root ?? "other";
}

export type QueryMatch = (query: Query) => boolean;

function startsWith(query: Query, prefix: QueryKeyParts): boolean {
	return prefix.every((part, index) => query.queryKey[index] === part);
}

// Resource identity: an omitted apiVersion matches on cluster, kind, namespace, and name.
function dependsOnResource(
	query: Query,
	families: ReadFamily[],
	resource: ResourceSummary,
	source: string | undefined,
	view: QueryKeyParts = [],
): boolean {
	const key = query.queryKey;
	return (
		families.some((family) => key[0] === family.root) &&
		key[1] === kubeconfigSourceKey(source) &&
		key[2] === resource.cluster &&
		(!resource.apiVersion || key[3] === resource.apiVersion) &&
		key[4] === resource.kind &&
		key[5] === (resource.namespace ?? "") &&
		key[6] === resource.name &&
		view.every((part, index) => key[7 + index] === part)
	);
}

export const queryMatchers = {
	/** Exactly the read with this catalog key; a scope with more parts is another read. */
	exact: (queryKey: QueryKeyParts): QueryMatch => (query) => matchQuery({ queryKey, exact: true }, query),
	/** Every read under a catalog scope key, such as `argoWorkspaceApplicationScope`. */
	prefix: (queryKey: QueryKeyParts): QueryMatch => (query) => matchQuery({ queryKey }, query),
	/** Active finite reads rendering one cluster context from one kubeconfig source. */
	currentView: (context: string, source?: string): QueryMatch => {
		const sourceKey = kubeconfigSourceKey(source);
		return (query) =>
			FAMILIES.get(query.queryKey[0])?.finite === true &&
			query.isActive() &&
			query.queryKey[1] === sourceKey &&
			query.queryKey[2] === context;
	},
	/** ADR 0006: the applied resource's details and YAML in the view that applied it. */
	yamlApply: (
		resource: ResourceSummary,
		source: string | undefined,
		{ dynamicKindKey, yamlViewMode, yamlEncoding }: Required<Omit<ResourceReadView, "redactSecrets">>,
	): QueryMatch => {
		const view = [dynamicKindKey ?? "", yamlViewMode, yamlEncoding];
		return (query) =>
			dependsOnResource(query, [F.resourceDetails, F.resourceYaml], resource, source, view);
	},
	/** ADR 0024: every details and YAML view and the revisions of the resource, plus lists and topology in its context. */
	resourceAction: (resource: ResourceSummary, source?: string): QueryMatch => {
		const lists = [F.resources, F.resourceTopology].map((family) =>
			scoped(family, source, resource.cluster),
		);
		return (query) =>
			dependsOnResource(
				query,
				[F.resourceDetails, F.resourceYaml, F.deploymentRevisions],
				resource,
				source,
			) || lists.some((prefix) => startsWith(query, prefix));
	},
	/** Cached resource lists in one context, whichever view loaded them. */
	resourceLists: (context: string, source?: string): QueryMatch => {
		const prefix = scoped(F.resources, source, context);
		return (query) => startsWith(query, prefix);
	},
	/** Every saved Argo CD connection status read. */
	argoConnectionStatuses: (): QueryMatch => (query) =>
		query.queryKey[0] === F.argoConnectionStatuses.root,
};
