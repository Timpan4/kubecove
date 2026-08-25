import type { TreeNodeId } from "./tree-nav";
import { SUPPORTED_KINDS } from "./types";
import type {
	DiscoveredResourceKind,
	JsonObject,
	JsonValue,
	ResourceKindSelection,
	ResourceSummary,
	SupportedKind,
	TopologyMode,
	YamlEncoding,
	YamlViewMode,
} from "./types";

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
	"attention",
	"degraded",
	"unknown",
	"notEvaluated",
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
	| "portForward"
	| "revisions"
	| "operations"
	| "argo";
export type PathStateIncidentFilter =
	| "all"
	| "degraded"
	| "attention"
	| "restarted"
	| "warning";
export type PathStateRbacRiskBucket =
	| "all"
	| "high"
	| "medium"
	| "low"
	| "none"
	| "unknown";

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
	rbac: {
		riskBucket: PathStateRbacRiskBucket;
		selectedObjectKey: string | null;
	} | null;
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

export interface PathStateSnapshot {
	version: typeof PATH_STATE_VERSION;
	runtime: "svelte";
	launcherView: PathStateLauncherView;
	workspace: PathStateWorkspaceSnapshot | null;
}

function isRecord<Value>(value: Value): value is Value & JsonObject {
	return value !== null && !Array.isArray(value) && Object(value) === value;
}

function isString<Value>(value: Value): value is Value & string {
	return String(value) === value;
}

function stringValue<Value>(value: Value, fallback = ""): string {
	return isString(value) ? value : fallback;
}

function nullableString<Value>(value: Value): string | null {
	return isString(value) ? value : null;
}

function optionalString<Value>(value: Value): string | undefined {
	return isString(value) ? value : undefined;
}

export function sanitizePathStateStringArray<Value>(
	value: Value,
	fallback: string[] = [],
): string[] {
	return Array.isArray(value) ? value.filter(isString) : fallback;
}

function isBoolean<Value>(value: Value): value is Value & boolean {
	return Boolean(value) === value;
}

function booleanValue<Value>(value: Value, fallback: boolean): boolean {
	return isBoolean(value) ? value : fallback;
}

function nonNegativeInteger<Value>(value: Value, fallback = 0): number {
	const number = Number(value);
	return Object.is(number, value) && Number.isInteger(number) && number >= 0
		? number
		: fallback;
}

function pickString<T extends string, Value>(value: Value, values: readonly T[], fallback: T): T {
	return values.find((candidate) => Object.is(candidate, value)) ?? fallback;
}

export function isPathStateWorkspaceViewMode<Value>(
	value: Value,
): value is Value & PathStateWorkspaceViewMode {
	return PATH_STATE_WORKSPACE_VIEW_MODES.some((mode) => Object.is(mode, value));
}

export function isPathStateHealthFilter<Value>(value: Value): value is Value & PathStateHealthFilter {
	return PATH_STATE_HEALTH_FILTERS.some((filter) => Object.is(filter, value));
}

function sanitizeDiscoveredResourceKind<Value>(value: Value): DiscoveredResourceKind | null {
	if (!isRecord(value)) return null;
	const group = stringValue(value.group);
	const version = stringValue(value.version);
	const apiVersion = stringValue(value.apiVersion);
	const kind = stringValue(value.kind);
	const plural = stringValue(value.plural);
	const shortNames = sanitizePathStateStringArray(value.shortNames);
	const namespaced = isBoolean(value.namespaced) ? value.namespaced : null;
	if (!version || !apiVersion || !kind || !plural || namespaced === null) return null;
	return { group, version, apiVersion, kind, plural, shortNames, namespaced };
}

function sanitizeResourceKindSelection<Value>(value: Value): ResourceKindSelection | null {
	const kind = stringValue(value);
	if (isSupportedKind(kind)) return kind;
	return sanitizeDiscoveredResourceKind(value);
}

function isSupportedKind(value: string): value is SupportedKind {
	return SUPPORTED_KINDS.some((kind) => kind === value);
}

function sanitizeResourceKinds<Value>(value: Value): ResourceKindSelection[] {
	if (!Array.isArray(value)) return [];
	return value.flatMap((item) => {
		const kind = sanitizeResourceKindSelection(item);
		return kind ? [kind] : [];
	});
}

export function sanitizePathStateTreeNode<Value>(value: Value): TreeNodeId | null {
	if (!isRecord(value)) return null;
	const type = pickString(value.type, ["section", "namespace", "group", "kind"] as const, "section");
	const section = stringValue(value.section);
	if (!section) return null;
	const resourceKind = sanitizeDiscoveredResourceKind(value.resourceKind);
	const node: TreeNodeId = {
		type,
		section,
		namespace: optionalString(value.namespace),
		group: optionalString(value.group),
		kind: optionalString(value.kind),
	};
	if (resourceKind) node.resourceKind = resourceKind;
	return node;
}

function sanitizeResourceRef<Value>(value: Value): PathStateResourceRef | null {
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
		namespaced: isBoolean(value.namespaced) ? value.namespaced : undefined,
		dynamic: isBoolean(value.dynamic) ? value.dynamic : undefined,
	};
}

function sanitizeTargetRef<Value>(value: Value): { name: string; namespace?: string | null } | null {
	if (!isRecord(value)) return null;
	const name = stringValue(value.name);
	if (!name) return null;
	return { name, namespace: nullableString(value.namespace) };
}

function sanitizeBrowserState<Value>(value: Value): PathStateResourceBrowserState | null {
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
		mapPanelOpen: booleanValue(value.mapPanelOpen, false),
		tablePanelOpen: booleanValue(value.tablePanelOpen, true),
	};
}

function sanitizeDetailState<Value>(value: Value): PathStateResourceDetailState | null {
	if (!isRecord(value)) return null;
	return {
		activeTab: pickString(
			value.activeTab,
			[
				"details",
				"yaml",
				"events",
				"logs",
				"exec",
				"portForward",
				"revisions",
				"operations",
				"argo",
			] as const,
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

function sanitizeSurfacesState<Value>(value: Value): PathStateSurfacesState | null {
	if (!isRecord(value)) return null;
	const rbac = isRecord(value.rbac)
		? {
				riskBucket: pickString(
					value.rbac.riskBucket,
					["all", "high", "medium", "low", "none", "unknown"] as const,
					"all",
				),
				selectedObjectKey: nullableString(value.rbac.selectedObjectKey),
			}
		: null;
	return {
		incidentFilter: pickString(
			value.incidentFilter,
			["all", "degraded", "attention", "restarted", "warning"] as const,
			"all",
		),
		helmSearch: stringValue(value.helmSearch),
		selectedHelmRelease: sanitizeTargetRef(value.selectedHelmRelease),
		selectedGitOpsApplication: nullableString(value.selectedGitOpsApplication),
		rbac,
	};
}

function sanitizeWorkspaceSnapshot<Value>(value: Value): PathStateWorkspaceSnapshot | null {
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

export function sanitizePathStateSnapshot<Value>(value: Value): PathStateSnapshot | null {
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

export function encodePathStateSnapshot(snapshot: PathStateSnapshot): string {
	return JSON.stringify(sanitizePathStateSnapshot(snapshot) ?? defaultPathStateSnapshot());
}

export function decodePathStateSnapshot(value: string | null | undefined): PathStateSnapshot | null {
	if (!value) return null;
	try {
		const parsed: JsonValue = JSON.parse(value);
		return sanitizePathStateSnapshot(parsed);
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
		healthAssessment: null,
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
	const hashSnapshot = globalThis.window === undefined ? null : parsePathStateHash(window.location.hash);
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
		return globalThis.window === undefined ? null : window.sessionStorage.getItem(key);
	} catch {
		return null;
	}
}

function writeSessionValue(key: string, value: string): void {
	try {
		if (globalThis.window !== undefined) window.sessionStorage.setItem(key, value);
	} catch {
		// sessionStorage can be unavailable in hardened WebViews; hash still carries coarse route.
	}
}

function replaceBrowserHash(hash: string): void {
	if (globalThis.window === undefined || window.location.hash === hash) return;
	try {
		window.history.replaceState(null, "", hash);
	} catch {
		window.location.hash = hash;
	}
}
