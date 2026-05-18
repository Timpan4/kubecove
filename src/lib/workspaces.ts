import { create } from "zustand";
import { persist } from "zustand/middleware";
import {
	CLUSTER_SCOPED_KINDS,
	SUPPORTED_KINDS,
	type ClusterContext,
	type DiscoveredResourceKind,
	type ResourceKindSelection,
	type ResourceSummary,
} from "./types";
import { classifyResourceHealth } from "./resource-health";

export type WorkspaceShortcutKind = "resources" | "namespace" | "argo";

export interface WorkspaceShortcut {
	id: string;
	label: string;
	kind: WorkspaceShortcutKind;
	namespace?: string;
	argoApp?: string;
}

export interface WorkspaceScope {
	clusterContext: string;
	namespaces: string[];
	kinds: ResourceKindSelection[];
	argoAppFilter: string;
	layout: "overview" | "resources";
}

export interface SavedWorkspace {
	id: string;
	name: string;
	createdAt: string;
	updatedAt: string;
	scope: WorkspaceScope;
	shortcuts: WorkspaceShortcut[];
}

export interface WorkspaceRestoreStatus {
	clusterAvailable: boolean;
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

interface CreateWorkspaceInput {
	name: string;
	clusterContext: string;
	namespaces: string[];
	kinds?: ResourceKindSelection[];
}

interface WorkspaceStore {
	workspaces: SavedWorkspace[];
	activeWorkspaceId: string | null;
	createWorkspace: (input: CreateWorkspaceInput) => SavedWorkspace;
	updateWorkspace: (
		id: string,
		updates: Partial<Pick<SavedWorkspace, "name" | "scope" | "shortcuts">>,
	) => void;
	deleteWorkspace: (id: string) => void;
	setActiveWorkspace: (id: string | null) => void;
}

export const DEFAULT_WORKSPACE_KINDS: ResourceKindSelection[] = [
	"Pod",
	"Deployment",
	"Service",
	"Ingress",
	"ConfigMap",
	"Secret",
];

function newWorkspaceId(): string {
	if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
		return crypto.randomUUID();
	}
	return `workspace-${Date.now().toString(36)}`;
}

export function resourceKindKey(kind: ResourceKindSelection): string {
	if (typeof kind === "string") return `builtin:${kind}`;
	return `dynamic:${kind.group}:${kind.version}:${kind.plural}:${kind.kind}`;
}

export function resourceKindLabel(kind: ResourceKindSelection): string {
	return typeof kind === "string" ? kind : kind.kind;
}

export function summarizeWorkspaceScope(scope: WorkspaceScope): string {
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
	return `${scope.clusterContext} / ${namespaceLabel} / ${kindLabel}`;
}

export function makeWorkspaceShortcuts(
	namespaces: string[],
	argoApp?: string,
): WorkspaceShortcut[] {
	const namespaceShortcuts = namespaces.slice(0, 4).map((namespace) => ({
		id: `namespace:${namespace}`,
		label: namespace,
		kind: "namespace" as const,
		namespace,
	}));
	const shortcuts: WorkspaceShortcut[] = [
		{ id: "resources:all", label: "Resources", kind: "resources" },
		...namespaceShortcuts,
	];
	if (argoApp) {
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
	const kinds = input.kinds?.length ? input.kinds : DEFAULT_WORKSPACE_KINDS;
	return {
		id: newWorkspaceId(),
		name,
		createdAt: now,
		updatedAt: now,
		scope: {
			clusterContext: input.clusterContext,
			namespaces: [...input.namespaces].sort((a, b) => a.localeCompare(b)),
			kinds,
			argoAppFilter: "",
			layout: "overview",
		},
		shortcuts: makeWorkspaceShortcuts(input.namespaces),
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
	const clusterAvailable = clusterContexts.some(
		(context) => context.name === workspace.scope.clusterContext,
	);
	const namespaceSet = new Set(namespaces);
	const kindSet = knownKindKeys(discoveredKinds);
	return {
		clusterAvailable,
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

export const useWorkspaceStore = create<WorkspaceStore>()(
	persist(
		(set) => ({
			workspaces: [],
			activeWorkspaceId: null,
			createWorkspace: (input) => {
				const workspace = createWorkspaceRecord(input);
				set((state) => ({ workspaces: [workspace, ...state.workspaces] }));
				return workspace;
			},
			updateWorkspace: (id, updates) =>
				set((state) => ({
					workspaces: state.workspaces.map((workspace) =>
						workspace.id === id
							? {
									...workspace,
									...updates,
									updatedAt: new Date().toISOString(),
								}
							: workspace,
					),
				})),
			deleteWorkspace: (id) =>
				set((state) => ({
					workspaces: state.workspaces.filter((workspace) => workspace.id !== id),
					activeWorkspaceId:
						state.activeWorkspaceId === id ? null : state.activeWorkspaceId,
				})),
			setActiveWorkspace: (id) => set({ activeWorkspaceId: id }),
		}),
		{
			name: "kubecove-workspaces",
			partialize: (state) => ({ workspaces: state.workspaces }),
		},
	),
);
