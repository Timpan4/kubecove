import {
	CLUSTER_SCOPED_KINDS,
	SUPPORTED_KINDS,
	type ClusterContext,
	type DiscoveredResourceKind,
	type ResourceKindSelection,
	type ResourceSummary,
} from "./types";
import { classifyResourceHealth } from "./resource-health";

export type WorkspaceShortcutKind = "resources" | "namespace" | "argo" | "compare";

export interface WorkspaceClusterGroup {
	id: string;
	name: string;
	members: string[];
}

export interface WorkspaceShortcutPreferences {
	includeResources: boolean;
	includeNamespaces: boolean;
	includeCompare: boolean;
	includeArgo: boolean;
}

export interface WorkspaceCompareEntry {
	id: string;
	kind: "contexts" | "namespaces";
	label: string;
	leftLabel: string;
	rightLabel: string;
	clusterContexts: string[];
	namespaces: string[];
}

export interface WorkspaceCompareSummary {
	entry: WorkspaceCompareEntry;
	left: WorkspaceHealthSummary;
	right: WorkspaceHealthSummary;
}

export interface WorkspaceShortcut {
	id: string;
	label: string;
	kind: WorkspaceShortcutKind;
	namespace?: string;
	argoApp?: string;
	compare?: WorkspaceCompareEntry;
}

export type SavedPortForwardLastStatus =
	| "idle"
	| "starting"
	| "listening"
	| "connected"
	| "error"
	| (string & {});

export interface SavedPortForward {
	id: string;
	clusterContext: string;
	namespace: string;
	serviceName: string;
	servicePort: number;
	localPort?: number;
	label?: string;
	createdAt: string;
	updatedAt: string;
	lastStartedAt?: string;
	lastStatus?: SavedPortForwardLastStatus;
	lastError?: string;
}

export interface SavePortForwardInput {
	clusterContext: string;
	namespace: string;
	serviceName: string;
	servicePort: number;
	localPort?: number;
	label?: string;
}

export type SavedPortForwardUpdates = Partial<
	Omit<SavedPortForward, "id" | "createdAt">
>;

export interface WorkspaceScope {
	clusterContext: string;
	clusterGroup?: WorkspaceClusterGroup;
	namespaces: string[];
	kinds: ResourceKindSelection[];
	gitOpsFilter?: string;
	argoAppFilter: string;
	layout: "overview" | "resources";
	shortcutPreferences?: WorkspaceShortcutPreferences;
}

export interface SavedWorkspace {
	id: string;
	sharedKey?: string;
	name: string;
	createdAt: string;
	updatedAt: string;
	scope: WorkspaceScope;
	shortcuts: WorkspaceShortcut[];
	portForwards: SavedPortForward[];
}

export interface WorkspaceRestoreStatus {
	clusterAvailable: boolean;
	missingClusterContexts: string[];
	missingNamespaces: string[];
	missingKinds: string[];
}

export interface WorkspaceHealthSummary {
	total: number;
	healthy: number;
	attention: number;
	degraded: number;
	restarted: number;
}

export interface CreateWorkspaceInput {
	name: string;
	clusterContext: string;
	clusterContexts?: string[];
	clusterGroupName?: string;
	namespaces: string[];
	kinds?: ResourceKindSelection[];
	shortcutPreferences?: Partial<WorkspaceShortcutPreferences>;
}

export interface CreateWorkspaceScopeInput extends CreateWorkspaceInput {
	name: string;
}

export const DEFAULT_WORKSPACE_KINDS: ResourceKindSelection[] = [
	"Pod",
	"Deployment",
	"ReplicaSet",
	"Job",
	"CronJob",
	"Service",
	"Ingress",
	"ConfigMap",
	"Secret",
];

export const DEFAULT_WORKSPACE_SHORTCUT_PREFERENCES: WorkspaceShortcutPreferences = {
	includeResources: true,
	includeNamespaces: true,
	includeCompare: true,
	includeArgo: true,
};

function newWorkspaceId(): string {
	if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
		return crypto.randomUUID();
	}
	return `workspace-${Date.now().toString(36)}`;
}

function newSavedPortForwardId(): string {
	if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
		return crypto.randomUUID();
	}
	return `port-forward-${Date.now().toString(36)}`;
}

function normalizeClusterContexts(
	primaryContext: string,
	clusterContexts?: string[],
): string[] {
	return Array.from(new Set([primaryContext, ...(clusterContexts ?? [])].filter(Boolean)));
}

function normalizeShortcutPreferences(
	preferences?: Partial<WorkspaceShortcutPreferences>,
): WorkspaceShortcutPreferences {
	return {
		...DEFAULT_WORKSPACE_SHORTCUT_PREFERENCES,
		...preferences,
	};
}

export function normalizeSavedPortForwardInput(
	input: SavePortForwardInput,
): SavePortForwardInput {
	const label = input.label?.trim();
	return {
		clusterContext: input.clusterContext.trim(),
		namespace: input.namespace.trim(),
		serviceName: input.serviceName.trim(),
		servicePort: input.servicePort,
		localPort: input.localPort,
		label: label || undefined,
	};
}

export function createSavedPortForward(
	input: SavePortForwardInput,
	now = new Date().toISOString(),
): SavedPortForward {
	return {
		id: newSavedPortForwardId(),
		...normalizeSavedPortForwardInput(input),
		createdAt: now,
		updatedAt: now,
		lastStatus: "idle",
	};
}

export function workspaceScopeContexts(scope: WorkspaceScope): string[] {
	return normalizeClusterContexts(
		scope.clusterContext,
		scope.clusterGroup?.members,
	);
}

export function reconcileSavedPortForwardsForScope(
	portForwards: SavedPortForward[],
	scope: WorkspaceScope,
): SavedPortForward[] {
	const contexts = workspaceScopeContexts(scope);
	return portForwards.filter(
		({ clusterContext }) => contexts.includes(clusterContext),
	);
}

export function resourceKindKey(kind: ResourceKindSelection): string {
	if (typeof kind === "string") return `builtin:${kind}`;
	return `dynamic:${kind.group}:${kind.version}:${kind.plural}:${kind.kind}`;
}

export function resourceKindLabel(kind: ResourceKindSelection): string {
	return typeof kind === "string" ? kind : kind.kind;
}

export function workspaceClusterGroupLabel(scope: WorkspaceScope): string {
	const contexts = workspaceScopeContexts(scope);
	if (contexts.length <= 1) return scope.clusterContext;
	return `${scope.clusterGroup?.name ?? "Cluster group"} (${contexts.length})`;
}

export function summarizeWorkspaceScope(scope: WorkspaceScope): string {
	const contextLabel = workspaceClusterGroupLabel(scope);
	const namespaceLabel =
		scope.namespaces.length === 0
			? "All namespaces"
			: scope.namespaces.length === 1
				? scope.namespaces[0]
				: `${scope.namespaces.slice(0, 2).join(", ")}${
						scope.namespaces.length > 2 ? ` +${scope.namespaces.length - 2}` : ""
					}`;
	const kindLabel =
		scope.kinds.length <= 2
			? scope.kinds.map(resourceKindLabel).join(", ")
			: `${scope.kinds.slice(0, 2).map(resourceKindLabel).join(", ")} +${
					scope.kinds.length - 2
				}`;
	return `${contextLabel} / ${namespaceLabel} / ${kindLabel}`;
}

export function buildWorkspaceCompareEntries(
	scope: WorkspaceScope,
): WorkspaceCompareEntry[] {
	const entries: WorkspaceCompareEntry[] = [];
	const contexts = workspaceScopeContexts(scope);
	if (contexts.length >= 2) {
		const [leftLabel, rightLabel] = contexts;
		entries.push({
			id: `ctx:${leftLabel}:${rightLabel}`,
			kind: "contexts",
			label: "Compare contexts",
			leftLabel,
			rightLabel,
			clusterContexts: [leftLabel, rightLabel],
			namespaces: scope.namespaces,
		});
	}
	if (scope.namespaces.length >= 2) {
		const [leftLabel, rightLabel] = scope.namespaces;
		entries.push({
			id: `namespaces:${leftLabel}:${rightLabel}`,
			kind: "namespaces",
			label: "Compare namespaces",
			leftLabel,
			rightLabel,
			clusterContexts: contexts,
			namespaces: [leftLabel, rightLabel],
		});
	}
	return entries;
}

export function makeWorkspaceShortcuts(
	namespaces: string[],
	argoApp?: string,
	preferences: WorkspaceShortcutPreferences = DEFAULT_WORKSPACE_SHORTCUT_PREFERENCES,
	scope?: WorkspaceScope,
): WorkspaceShortcut[] {
	const shortcuts: WorkspaceShortcut[] = [];
	if (preferences.includeResources) {
		shortcuts.push({ id: "resources:all", label: "Resources", kind: "resources" });
	}
	if (preferences.includeNamespaces) {
		shortcuts.push(
			...namespaces.slice(0, 4).map((namespace) => ({
				id: `namespace:${namespace}`,
				label: namespace,
				kind: "namespace" as const,
				namespace,
			})),
		);
	}
	const compare = scope ? buildWorkspaceCompareEntries(scope)[0] : null;
	if (preferences.includeCompare && compare) {
		shortcuts.push({
			id: `compare:${compare.id}`,
			label: "Compare",
			kind: "compare",
			compare,
		});
	}
	if (preferences.includeArgo && argoApp) {
		shortcuts.push({
			id: `argo:${argoApp}`,
			label: argoApp,
			kind: "argo",
			argoApp,
		});
	}
	return shortcuts;
}

export function createWorkspaceRecord(
	input: CreateWorkspaceInput,
	now = new Date().toISOString(),
): SavedWorkspace {
	const name = input.name.trim() || input.clusterContext;
	const scope = createWorkspaceScope({ ...input, name });
	const shortcutPreferences = normalizeShortcutPreferences(input.shortcutPreferences);
	return {
		id: newWorkspaceId(),
		name,
		createdAt: now,
		updatedAt: now,
		scope,
		shortcuts: makeWorkspaceShortcuts(scope.namespaces, undefined, shortcutPreferences, scope),
		portForwards: [],
	};
}

export function createWorkspaceScope(
	input: CreateWorkspaceScopeInput,
): WorkspaceScope {
	const name = input.name.trim() || input.clusterContext;
	const kinds = input.kinds?.length ? input.kinds : DEFAULT_WORKSPACE_KINDS;
	const contexts = normalizeClusterContexts(input.clusterContext, input.clusterContexts);
	const shortcutPreferences = normalizeShortcutPreferences(input.shortcutPreferences);
	const clusterGroupName = input.clusterGroupName?.trim() || `${name} contexts`;
	return {
		clusterContext: input.clusterContext,
		clusterGroup:
			contexts.length > 1 || input.clusterGroupName?.trim()
				? {
						id: `cluster-group:${contexts.join("|")}`,
						name: clusterGroupName,
						members: contexts,
					}
				: undefined,
		namespaces: [...input.namespaces].sort((a, b) => a.localeCompare(b)),
		kinds,
		gitOpsFilter: "",
		argoAppFilter: "",
		layout: "overview",
		shortcutPreferences,
	};
}

export function updateWorkspaceRecord(
	workspace: SavedWorkspace,
	input: CreateWorkspaceInput,
	now = new Date().toISOString(),
): SavedWorkspace {
	const name = input.name.trim() || input.clusterContext;
	const scope = createWorkspaceScope({
		...input,
		name,
		kinds: input.kinds ?? workspace.scope.kinds,
		shortcutPreferences:
			input.shortcutPreferences ?? workspace.scope.shortcutPreferences,
	});
	const nextScope: WorkspaceScope = {
		...scope,
		gitOpsFilter: workspace.scope.gitOpsFilter ?? workspace.scope.argoAppFilter,
		argoAppFilter: workspace.scope.argoAppFilter,
		layout: workspace.scope.layout,
	};
	return {
		...workspace,
		name,
		scope: nextScope,
		shortcuts: makeWorkspaceShortcuts(
			nextScope.namespaces,
			undefined,
			nextScope.shortcutPreferences,
			nextScope,
		),
		portForwards: reconcileSavedPortForwardsForScope(
			workspace.portForwards ?? [],
			nextScope,
		),
		updatedAt: now,
	};
}

function knownKindKeys(discoveredKinds: DiscoveredResourceKind[]): Set<string> {
	return new Set([
		...(SUPPORTED_KINDS as readonly string[]).map((kind) => `builtin:${kind}`),
		...(CLUSTER_SCOPED_KINDS as readonly string[]).map((kind) => `builtin:${kind}`),
		...discoveredKinds.map(resourceKindKey),
	]);
}

export function computeRestoreStatus(
	workspace: SavedWorkspace,
	clusterContexts: ClusterContext[],
	namespaces: string[],
	discoveredKinds: DiscoveredResourceKind[],
): WorkspaceRestoreStatus {
	const availableContexts = new Set(clusterContexts.map((context) => context.name));
	const missingClusterContexts = workspaceScopeContexts(workspace.scope).filter(
		(context) => !availableContexts.has(context),
	);
	const clusterAvailable = availableContexts.has(workspace.scope.clusterContext);
	const namespaceSet = new Set(namespaces);
	const kindSet = knownKindKeys(discoveredKinds);
	return {
		clusterAvailable,
		missingClusterContexts,
		missingNamespaces: workspace.scope.namespaces.filter(
			(namespace) => !namespaceSet.has(namespace),
		),
		missingKinds: workspace.scope.kinds
			.filter((kind) => !kindSet.has(resourceKindKey(kind)))
			.map(resourceKindLabel),
	};
}

export function buildWorkspaceHealthSummary(
	rows: ResourceSummary[],
): WorkspaceHealthSummary {
	return rows.reduce<WorkspaceHealthSummary>(
		(summary, row) => {
			const health = classifyResourceHealth(row);
			return {
				total: summary.total + 1,
				healthy: summary.healthy + (health.healthy ? 1 : 0),
				attention: summary.attention + (health.attention ? 1 : 0),
				degraded: summary.degraded + (health.degraded ? 1 : 0),
				restarted: summary.restarted + (health.restarted ? 1 : 0),
			};
		},
		{ total: 0, healthy: 0, attention: 0, degraded: 0, restarted: 0 },
	);
}

export function buildWorkspaceCompareSummaries(
	entries: WorkspaceCompareEntry[],
	rows: ResourceSummary[],
): WorkspaceCompareSummary[] {
	return entries.map((entry) => {
		const leftRows = rows.filter((row) =>
			entry.kind === "contexts"
				? row.cluster === entry.leftLabel
				: row.namespace === entry.leftLabel,
		);
		const rightRows = rows.filter((row) =>
			entry.kind === "contexts"
				? row.cluster === entry.rightLabel
				: row.namespace === entry.rightLabel,
		);
		return {
			entry,
			left: buildWorkspaceHealthSummary(leftRows),
			right: buildWorkspaceHealthSummary(rightRows),
		};
	});
}
