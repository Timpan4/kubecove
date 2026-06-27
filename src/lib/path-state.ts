import type {
	DiscoveredResourceKind,
	ResourceKindSelection,
	ResourceSummary,
	TopologyMode,
	YamlEncoding,
	YamlViewMode,
} from "./types";
import type { TreeNodeId } from "./tree-nav";

export const PATH_STATE_SESSION_KEY = "kubecove-path-state-v1";

const PATH_STATE_VERSION = 1;

export const PATH_STATE_WORKSPACE_VIEW_MODES = [
	"overview",
	"resources",
	"argo",
	"helm",
	"incidents",
	"portForwards",
	"rbac",
	"settings",
] as const;
export const PATH_STATE_HEALTH_FILTERS = [
	"all",
	"healthy",
	"unhealthy",
	"attention",
	"degraded",
	"restarted",
] as const;

export type PathStateLauncherView = "workspaces" | "settings";
export type PathStateWorkspaceViewMode =
	(typeof PATH_STATE_WORKSPACE_VIEW_MODES)[number];
export type PathStateHealthFilter = (typeof PATH_STATE_HEALTH_FILTERS)[number];
export type PathStateResourceSortColumn =
	| "name"
	| "namespace"
	| "kind"
	| "status"
	| "ready"
	| "restarts"
	| "age"
	| "cpu"
	| "memory";
export type PathStateDetailTab =
	| "details"
	| "yaml"
	| "events"
	| "logs"
	| "exec"
	| "portForward";
export type PathStateIncidentFilter =
	| "all"
	| "unhealthy"
	| "degraded"
	| "attention"
	| "restarted"
	| "warning";

export interface PathStateResourceRef {
	cluster: string;
	kind: string;
	name: string;
	namespace: string | null;
	apiVersion?: string;
	group?: string;
	version?: string;
	plural?: string;
	namespaced?: boolean;
	dynamic?: boolean;
}

export interface PathStateResourceBrowserState {
	selectedNamespaces: string[];
	selectedKinds: ResourceKindSelection[];
	search: string;
	gitOpsFilter: string;
	healthFilter: PathStateHealthFilter;
	sortColumn: PathStateResourceSortColumn;
	sortDesc: boolean;
	pageIndex: number;
	scopeEditorOpen: boolean;
	collapsedGroups: string[];
	topologyMode: TopologyMode;
	selectedTopologyNodeId: string | null;
	mapPanelOpen: boolean;
	tablePanelOpen: boolean;
}

export interface PathStateResourceDetailState {
	activeTab: PathStateDetailTab;
	metadataLabelsExpanded: boolean;
	metadataAnnotationsExpanded: boolean;
	selectedContainer: string;
	logFilter: string;
	logWrapLines: boolean;
	logLatestFirst: boolean;
	logAutoFollow: boolean;
	yamlViewMode: YamlViewMode;
	yamlEncoding: YamlEncoding;
	yamlShowFullDiff: boolean;
}

export interface PathStateSurfacesState {
	incidentFilter: PathStateIncidentFilter;
	helmSearch: string;
	selectedHelmRelease: { name: string; namespace?: string | null } | null;
	selectedGitOpsApplication: string | null;
}

export interface PathStateWorkspaceSnapshot {
	workspaceId: string;
	viewMode: PathStateWorkspaceViewMode;
	selectedNode: TreeNodeId | null;
	expandedSections: string[];
	resourceInitialSearch: string;
	resourceInitialGitOpsFilter: string;
	resourceInitialHealthFilter: PathStateHealthFilter;
	resourceNamespaceOverride: string[] | null;
	focusedResource: PathStateResourceRef | null;
	restoreTargetResource: PathStateResourceRef | null;
	targetHelmRelease: { name: string; namespace?: string | null } | null;
	targetGitOpsApplication: string | null;
	resources: PathStateResourceBrowserState | null;
	detail: PathStateResourceDetailState | null;
	surfaces: PathStateSurfacesState | null;
}

export interface PathStateWorkspaceHandoff {
	workspaceId: string;
	selectedNode?: TreeNodeId | null;
	expandedSections?: string[];
	viewMode?: PathStateWorkspaceViewMode;
	resourceInitialSearch?: string;
	resourceInitialGitOpsFilter?: string;
	resourceInitialHealthFilter?: PathStateHealthFilter;
	resourceNamespaceOverride?: string[] | null;
}

export interface PathStateSnapshot {
	version: typeof PATH_STATE_VERSION;
	runtime: "svelte";
	launcherView: PathStateLauncherView;
	workspace: PathStateWorkspaceSnapshot | null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringValue(value: unknown, fallback = ""): string {
	return typeof value === "string" ? value : fallback;
}

function nullableString(value: unknown): string | null {
	return typeof value === "string" ? value : null;
}

function optionalString(value: unknown): string | undefined {
	return typeof value === "string" ? value : undefined;
}

export function sanitizePathStateStringArray(
	value: unknown,
	fallback: string[] = [],
): string[] {
	return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : fallback;
}

function booleanValue(value: unknown, fallback: boolean): boolean {
	return typeof value === "boolean" ? value : fallback;
}

function nonNegativeInteger(value: unknown, fallback = 0): number {
	return typeof value === "number" && Number.isInteger(value) && value >= 0
		? value
		: fallback;
}

function pickString<T extends string>(value: unknown, values: readonly T[], fallback: T): T {
	return typeof value === "string" && (values as readonly string[]).includes(value)
		? (value as T)
		: fallback;
}

export function isPathStateWorkspaceViewMode(
	value: unknown,
): value is PathStateWorkspaceViewMode {
	return (
		typeof value === "string" &&
		(PATH_STATE_WORKSPACE_VIEW_MODES as readonly string[]).includes(value)
	);
}

export function isPathStateHealthFilter(value: unknown): value is PathStateHealthFilter {
	return (
		typeof value === "string" &&
		(PATH_STATE_HEALTH_FILTERS as readonly string[]).includes(value)
	);
}

function sanitizeDiscoveredResourceKind(value: unknown): DiscoveredResourceKind | null {
	if (!isRecord(value)) return null;
	const group = stringValue(value.group);
	const version = stringValue(value.version);
	const apiVersion = stringValue(value.apiVersion);
	const kind = stringValue(value.kind);
	const plural = stringValue(value.plural);
	const namespaced = typeof value.namespaced === "boolean" ? value.namespaced : null;
	if (!version || !apiVersion || !kind || !plural || namespaced === null) return null;
	return { group, version, apiVersion, kind, plural, namespaced };
}

function sanitizeResourceKindSelection(value: unknown): ResourceKindSelection | null {
	if (typeof value === "string" && value.trim()) return value as ResourceKindSelection;
	return sanitizeDiscoveredResourceKind(value);
}

function sanitizeResourceKinds(value: unknown): ResourceKindSelection[] {
	if (!Array.isArray(value)) return [];
	return value.flatMap((item) => {
		const kind = sanitizeResourceKindSelection(item);
		return kind ? [kind] : [];
	});
}

export function sanitizePathStateTreeNode(value: unknown): TreeNodeId | null {
	if (!isRecord(value)) return null;
	const type = pickString(value.type, ["section", "namespace", "group", "kind"] as const, "section");
	const section = stringValue(value.section);
	if (!section) return null;
	const resourceKind = sanitizeDiscoveredResourceKind(value.resourceKind);
	return {
		type,
		section,
		namespace: optionalString(value.namespace),
		group: optionalString(value.group),
		kind: optionalString(value.kind),
		...(resourceKind ? { resourceKind } : {}),
	};
}

function sanitizeResourceRef(value: unknown): PathStateResourceRef | null {
	if (!isRecord(value)) return null;
	const cluster = stringValue(value.cluster);
	const kind = stringValue(value.kind);
	const name = stringValue(value.name);
	if (!cluster || !kind || !name) return null;
	return {
		cluster,
		kind,
		name,
		namespace: nullableString(value.namespace),
		apiVersion: optionalString(value.apiVersion),
		group: optionalString(value.group),
		version: optionalString(value.version),
		plural: optionalString(value.plural),
		namespaced: typeof value.namespaced === "boolean" ? value.namespaced : undefined,
		dynamic: typeof value.dynamic === "boolean" ? value.dynamic : undefined,
	};
}

function sanitizeTargetRef(value: unknown): { name: string; namespace?: string | null } | null {
	if (!isRecord(value)) return null;
	const name = stringValue(value.name);
	if (!name) return null;
	return { name, namespace: nullableString(value.namespace) };
}

function sanitizeBrowserState(value: unknown): PathStateResourceBrowserState | null {
	if (!isRecord(value)) return null;
	return {
		selectedNamespaces: sanitizePathStateStringArray(value.selectedNamespaces),
		selectedKinds: sanitizeResourceKinds(value.selectedKinds),
		search: stringValue(value.search),
		gitOpsFilter: stringValue(value.gitOpsFilter),
		healthFilter: pickString(
			value.healthFilter,
			PATH_STATE_HEALTH_FILTERS,
			"all",
		),
		sortColumn: pickString(
			value.sortColumn,
			["name", "namespace", "kind", "status", "ready", "restarts", "age", "cpu", "memory"] as const,
			"name",
		),
		sortDesc: booleanValue(value.sortDesc, false),
		pageIndex: nonNegativeInteger(value.pageIndex),
		scopeEditorOpen: booleanValue(value.scopeEditorOpen, false),
		collapsedGroups: sanitizePathStateStringArray(value.collapsedGroups),
		topologyMode: pickString(value.topologyMode, ["ownership", "networkFlow"] as const, "ownership"),
		selectedTopologyNodeId: nullableString(value.selectedTopologyNodeId),
		mapPanelOpen: booleanValue(value.mapPanelOpen, true),
		tablePanelOpen: booleanValue(value.tablePanelOpen, true),
	};
}

function sanitizeDetailState(value: unknown): PathStateResourceDetailState | null {
	if (!isRecord(value)) return null;
	return {
		activeTab: pickString(
			value.activeTab,
			["details", "yaml", "events", "logs", "exec", "portForward"] as const,
			"details",
		),
		metadataLabelsExpanded: booleanValue(value.metadataLabelsExpanded, false),
		metadataAnnotationsExpanded: booleanValue(value.metadataAnnotationsExpanded, false),
		selectedContainer: stringValue(value.selectedContainer),
		logFilter: stringValue(value.logFilter),
		logWrapLines: booleanValue(value.logWrapLines, true),
		logLatestFirst: booleanValue(value.logLatestFirst, false),
		logAutoFollow: booleanValue(value.logAutoFollow, true),
		yamlViewMode: pickString(value.yamlViewMode, ["kubectl", "applyClean"] as const, "kubectl"),
		yamlEncoding: pickString(value.yamlEncoding, ["yaml", "kyaml"] as const, "yaml"),
		yamlShowFullDiff: booleanValue(value.yamlShowFullDiff, false),
	};
}

function sanitizeSurfacesState(value: unknown): PathStateSurfacesState | null {
	if (!isRecord(value)) return null;
	return {
		incidentFilter: pickString(
			value.incidentFilter,
			["all", "unhealthy", "degraded", "attention", "restarted", "warning"] as const,
			"all",
		),
		helmSearch: stringValue(value.helmSearch),
		selectedHelmRelease: sanitizeTargetRef(value.selectedHelmRelease),
		selectedGitOpsApplication: nullableString(value.selectedGitOpsApplication),
	};
}

function sanitizeWorkspaceSnapshot(value: unknown): PathStateWorkspaceSnapshot | null {
	if (!isRecord(value)) return null;
	const workspaceId = stringValue(value.workspaceId);
	if (!workspaceId) return null;
	const focusedResource = sanitizeResourceRef(value.focusedResource);
	const restoreTargetResource = sanitizeResourceRef(value.restoreTargetResource) ?? focusedResource;
	return {
		workspaceId,
		viewMode: pickString(
			value.viewMode,
			PATH_STATE_WORKSPACE_VIEW_MODES,
			"overview",
		),
		selectedNode: sanitizePathStateTreeNode(value.selectedNode),
		expandedSections: sanitizePathStateStringArray(value.expandedSections),
		resourceInitialSearch: stringValue(value.resourceInitialSearch),
		resourceInitialGitOpsFilter: stringValue(value.resourceInitialGitOpsFilter),
		resourceInitialHealthFilter: pickString(
			value.resourceInitialHealthFilter,
			PATH_STATE_HEALTH_FILTERS,
			"all",
		),
		resourceNamespaceOverride: Array.isArray(value.resourceNamespaceOverride)
			? sanitizePathStateStringArray(value.resourceNamespaceOverride)
			: null,
		focusedResource,
		restoreTargetResource,
		targetHelmRelease: sanitizeTargetRef(value.targetHelmRelease),
		targetGitOpsApplication: nullableString(value.targetGitOpsApplication),
		resources: sanitizeBrowserState(value.resources),
		detail: sanitizeDetailState(value.detail),
		surfaces: sanitizeSurfacesState(value.surfaces),
	};
}

export function sanitizePathStateSnapshot(value: unknown): PathStateSnapshot | null {
	if (!isRecord(value) || value.version !== PATH_STATE_VERSION || value.runtime !== "svelte") {
		return null;
	}
	return {
		version: PATH_STATE_VERSION,
		runtime: "svelte",
		launcherView: pickString(value.launcherView, ["workspaces", "settings"] as const, "workspaces"),
		workspace: sanitizeWorkspaceSnapshot(value.workspace),
	};
}

export function sanitizePathStateWorkspaceHandoff(
	value: unknown,
): PathStateWorkspaceHandoff | null {
	if (typeof value === "string") return value ? { workspaceId: value } : null;
	if (!isRecord(value)) return null;
	const workspaceId = stringValue(value.workspaceId);
	if (!workspaceId) return null;
	const handoff: PathStateWorkspaceHandoff = { workspaceId };
	if ("selectedNode" in value) {
		if (value.selectedNode === null) {
			handoff.selectedNode = null;
		} else {
			const selectedNode = sanitizePathStateTreeNode(value.selectedNode);
			if (selectedNode) handoff.selectedNode = selectedNode;
		}
	}
	if (Array.isArray(value.expandedSections)) {
		handoff.expandedSections = sanitizePathStateStringArray(value.expandedSections);
	}
	if (isPathStateWorkspaceViewMode(value.viewMode)) {
		handoff.viewMode = value.viewMode;
	}
	const resourceInitialSearch = optionalString(value.resourceInitialSearch);
	if (resourceInitialSearch !== undefined) {
		handoff.resourceInitialSearch = resourceInitialSearch;
	}
	const resourceInitialGitOpsFilter = optionalString(value.resourceInitialGitOpsFilter);
	if (resourceInitialGitOpsFilter !== undefined) {
		handoff.resourceInitialGitOpsFilter = resourceInitialGitOpsFilter;
	}
	if (isPathStateHealthFilter(value.resourceInitialHealthFilter)) {
		handoff.resourceInitialHealthFilter = value.resourceInitialHealthFilter;
	}
	if ("resourceNamespaceOverride" in value) {
		handoff.resourceNamespaceOverride =
			value.resourceNamespaceOverride === null
				? null
				: sanitizePathStateStringArray(value.resourceNamespaceOverride);
	}
	return handoff;
}

export function decodePathStateWorkspaceHandoff(
	raw: string | null | undefined,
): PathStateWorkspaceHandoff | null {
	if (!raw) return null;
	try {
		return sanitizePathStateWorkspaceHandoff(JSON.parse(raw));
	} catch {
		return sanitizePathStateWorkspaceHandoff(raw);
	}
}

export function encodePathStateSnapshot(snapshot: PathStateSnapshot): string {
	return JSON.stringify(sanitizePathStateSnapshot(snapshot) ?? defaultPathStateSnapshot());
}

export function decodePathStateSnapshot(value: string | null | undefined): PathStateSnapshot | null {
	if (!value) return null;
	try {
		return sanitizePathStateSnapshot(JSON.parse(value));
	} catch {
		return null;
	}
}

export function resourceRefFromSummary(resource: ResourceSummary): PathStateResourceRef {
	return {
		cluster: resource.cluster,
		kind: resource.kind,
		name: resource.name,
		namespace: resource.namespace,
		apiVersion: resource.apiVersion,
		group: resource.group,
		version: resource.version,
		plural: resource.plural,
		namespaced: resource.namespaced,
		dynamic: resource.dynamic,
	};
}

export function resourceSummaryFromRef(ref: PathStateResourceRef): ResourceSummary {
	return {
		cluster: ref.cluster,
		kind: ref.kind,
		name: ref.name,
		namespace: ref.namespace,
		age: "",
		health: "unknown",
		apiVersion: ref.apiVersion,
		group: ref.group,
		version: ref.version,
		plural: ref.plural,
		namespaced: ref.namespaced,
		dynamic: ref.dynamic,
	};
}

export function pathForPathState(snapshot: Pick<PathStateSnapshot, "launcherView" | "workspace">): string {
	if (!snapshot.workspace) return snapshot.launcherView === "settings" ? "#/settings" : "#/workspaces";
	return `#/workspace/${encodeURIComponent(snapshot.workspace.workspaceId)}/${encodeURIComponent(snapshot.workspace.viewMode)}`;
}

export function parsePathStateHash(hash: string): PathStateSnapshot | null {
	const path = hash.startsWith("#") ? hash.slice(1) : hash;
	const parts = path.split("/").filter(Boolean);
	if (parts.length === 1 && parts[0] === "workspaces") return defaultPathStateSnapshot("workspaces");
	if (parts.length === 1 && parts[0] === "settings") return defaultPathStateSnapshot("settings");
	if (parts.length >= 3 && parts[0] === "workspace") {
		const workspaceId = decodeURIComponent(parts[1] ?? "");
		if (!workspaceId) return null;
		return {
			...defaultPathStateSnapshot(),
			workspace: {
				...defaultWorkspaceSnapshot(workspaceId),
				viewMode: pickString(
					decodeURIComponent(parts[2] ?? ""),
					PATH_STATE_WORKSPACE_VIEW_MODES,
					"overview",
				),
			},
		};
	}
	return null;
}

export function readPathState(): PathStateSnapshot | null {
	const hashSnapshot = typeof window === "undefined" ? null : parsePathStateHash(window.location.hash);
	const storageSnapshot = decodePathStateSnapshot(readSessionValue(PATH_STATE_SESSION_KEY));
	if (!hashSnapshot) return storageSnapshot;
	if (!hashSnapshot.workspace) return hashSnapshot;
	if (storageSnapshot?.workspace?.workspaceId === hashSnapshot.workspace.workspaceId) return storageSnapshot;
	return hashSnapshot;
}

export function writePathState(snapshot: PathStateSnapshot): void {
	const safeSnapshot = sanitizePathStateSnapshot(snapshot);
	if (!safeSnapshot) return;
	writeSessionValue(PATH_STATE_SESSION_KEY, JSON.stringify(safeSnapshot));
	replaceBrowserHash(pathForPathState(safeSnapshot));
}

export function defaultPathStateSnapshot(
	launcherView: PathStateLauncherView = "workspaces",
): PathStateSnapshot {
	return {
		version: PATH_STATE_VERSION,
		runtime: "svelte",
		launcherView,
		workspace: null,
	};
}

function defaultWorkspaceSnapshot(workspaceId: string): PathStateWorkspaceSnapshot {
	return {
		workspaceId,
		viewMode: "overview",
		selectedNode: null,
		expandedSections: [],
		resourceInitialSearch: "",
		resourceInitialGitOpsFilter: "",
		resourceInitialHealthFilter: "all",
		resourceNamespaceOverride: null,
		focusedResource: null,
		restoreTargetResource: null,
		targetHelmRelease: null,
		targetGitOpsApplication: null,
		resources: null,
		detail: null,
		surfaces: null,
	};
}

function readSessionValue(key: string): string | null {
	try {
		return typeof window === "undefined" ? null : window.sessionStorage.getItem(key);
	} catch {
		return null;
	}
}

function writeSessionValue(key: string, value: string): void {
	try {
		if (typeof window !== "undefined") window.sessionStorage.setItem(key, value);
	} catch {
		// sessionStorage can be unavailable in hardened WebViews; hash still carries coarse route.
	}
}

function replaceBrowserHash(hash: string): void {
	if (typeof window === "undefined" || window.location.hash === hash) return;
	try {
		window.history.replaceState(null, "", hash);
	} catch {
		window.location.hash = hash;
	}
}
