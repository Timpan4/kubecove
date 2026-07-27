import { derived, get, type Readable, writable } from "svelte/store";
import type { ResourceSummary } from "@/lib/types";
import {
	entryPointFromApplication,
	entryPointFromNamespace,
	entryPointFromResource,
	entryPointsEqual,
	normalizeEntryPoints,
	type ResourceEntryPointCoverage,
	reconcileEntryPoints,
	recordRecentEntry,
	togglePinnedEntry,
} from "@/lib/workspace-entry-points";
import {
	type CreateWorkspaceInput,
	createSavedPortForward,
	createWorkspaceRecord,
	normalizeSavedPortForwardInput,
	reconcileSavedPortForwardsForScope,
	type SavedPortForwardUpdates,
	type SavedWorkspace,
	type SavePortForwardInput,
	updateWorkspaceRecord,
} from "@/lib/workspace-model";
import {
	applyWorkspaceImport,
	type WorkspaceImportDecisions,
	type WorkspaceImportPreview,
	type WorkspaceImportResult,
	} from "./workspace-sharing";

const WORKSPACE_STORAGE_KEY = "kubecove-workspaces";

interface StorageLike {
	getItem: (key: string) => string | null;
	setItem: (key: string, value: string) => void;
}

interface PersistedWorkspaceState {
	state?: {
		workspaces?: unknown;
	};
	version?: unknown;
}

interface WorkspaceState {
	workspaces: SavedWorkspace[];
	selectedWorkspaceId: string | null;
}

export interface WorkspaceStore {
	subscribe: Readable<WorkspaceState>["subscribe"];
	workspaces: Readable<SavedWorkspace[]>;
	selectedWorkspaceId: Readable<string | null>;
	selectedWorkspace: Readable<SavedWorkspace | null>;
	refreshFromStorage: () => void;
	createWorkspace: (input: CreateWorkspaceInput) => SavedWorkspace;
	updateWorkspace: (id: string, input: CreateWorkspaceInput) => void;
	saveSavedPortForward: (
		workspaceId: string,
		input: SavePortForwardInput,
	) => void;
	updateSavedPortForward: (
		workspaceId: string,
		portForwardId: string,
		updates: SavedPortForwardUpdates,
	) => void;
	deleteSavedPortForward: (workspaceId: string, portForwardId: string) => void;
	togglePinnedResource: (workspaceId: string, resource: ResourceSummary) => void;
	recordRecentNamespace: (workspaceId: string, clusterContext: string, namespace: string) => void;
	recordRecentApplication: (
		workspaceId: string,
		clusterContext: string,
		name: string,
		namespace?: string | null,
	) => void;
	recordRecentResource: (workspaceId: string, resource: ResourceSummary) => void;
	reconcileEntryPoints: (
		workspaceId: string,
		resources: ResourceSummary[],
		coverage: ResourceEntryPointCoverage[],
	) => void;
	importWorkspaces: (
		preview: WorkspaceImportPreview,
		decisions: WorkspaceImportDecisions,
	) => WorkspaceImportResult;
	deleteWorkspace: (id: string) => void;
	openWorkspace: (id: string) => void;
	clearSelectedWorkspace: () => void;
}

function browserStorage(): StorageLike | null {
	return typeof localStorage === "undefined" ? null : localStorage;
}

export function readPersistedWorkspaces(
	storage: StorageLike | null = browserStorage(),
): SavedWorkspace[] {
	if (!storage) return [];
	const raw = storage.getItem(WORKSPACE_STORAGE_KEY);
	if (!raw) return [];
	try {
		const parsed = JSON.parse(raw) as PersistedWorkspaceState;
		return Array.isArray(parsed.state?.workspaces)
			? (parsed.state.workspaces as SavedWorkspace[])
			: [];
	} catch {
		return [];
	}
}

export function writePersistedWorkspaces(
	workspaces: SavedWorkspace[],
	storage: StorageLike | null = browserStorage(),
): void {
	if (!storage) return;
	storage.setItem(
		WORKSPACE_STORAGE_KEY,
		JSON.stringify({ state: { workspaces }, version: 0 }),
	);
}

export function createWorkspaceStore(
	storage: StorageLike | null = browserStorage(),
): WorkspaceStore {
	const state = writable<WorkspaceState>({
		workspaces: readPersistedWorkspaces(storage),
		selectedWorkspaceId: null,
	});

	function updateAndPersist(
		recipe: (current: WorkspaceState) => WorkspaceState,
	) {
		state.update((current) => {
			const next = recipe(current);
			writePersistedWorkspaces(next.workspaces, storage);
			return next;
		});
	}

	function updateWorkspaceEntryPoints(
		workspaceId: string,
		recipe: (
			entryPoints: ReturnType<typeof normalizeEntryPoints>,
			now: string,
		) => ReturnType<typeof normalizeEntryPoints>,
	) {
		const current = get(state);
		const now = new Date().toISOString();
		let changed = false;
		const workspaces = current.workspaces.map((workspace) => {
			if (workspace.id !== workspaceId) return workspace;
			const currentEntryPoints = normalizeEntryPoints(workspace.entryPoints);
			const nextEntryPoints = recipe(currentEntryPoints, now);
			if (entryPointsEqual(currentEntryPoints, nextEntryPoints)) return workspace;
			changed = true;
			return {
				...workspace,
				entryPoints: nextEntryPoints,
				updatedAt: now,
			};
		});
		if (!changed) return;
		writePersistedWorkspaces(workspaces, storage);
		state.set({ ...current, workspaces });
	}

	return {
		subscribe: state.subscribe,
		workspaces: derived(state, ($state) => $state.workspaces),
		selectedWorkspaceId: derived(state, ($state) => $state.selectedWorkspaceId),
		selectedWorkspace: derived(state, ($state) => {
			return (
				$state.workspaces.find(
					(workspace) => workspace.id === $state.selectedWorkspaceId,
				) ?? null
			);
		}),
		refreshFromStorage: () => {
			state.set({
				workspaces: readPersistedWorkspaces(storage),
				selectedWorkspaceId: null,
			});
		},
		createWorkspace: (input) => {
			const workspace = createWorkspaceRecord(input);
			updateAndPersist((current) => ({
				workspaces: [workspace, ...current.workspaces],
				selectedWorkspaceId: workspace.id,
			}));
			return workspace;
		},
		updateWorkspace: (id, input) => {
			updateAndPersist((current) => ({
				...current,
				workspaces: current.workspaces.map((workspace) =>
					workspace.id === id ? updateWorkspaceRecord(workspace, input) : workspace,
				),
			}));
		},
		saveSavedPortForward: (workspaceId, input) => {
			const savedPortForward = createSavedPortForward(input);
			updateAndPersist((current) => ({
				...current,
				workspaces: current.workspaces.map((workspace) =>
					workspace.id === workspaceId
						? {
								...workspace,
								portForwards: reconcileSavedPortForwardsForScope(
									[savedPortForward, ...(workspace.portForwards ?? [])],
									workspace.scope,
								),
								updatedAt: savedPortForward.updatedAt,
							}
						: workspace,
				),
			}));
		},
		updateSavedPortForward: (workspaceId, portForwardId, updates) => {
			updateAndPersist((current) => {
				const now = new Date().toISOString();
				return {
					...current,
					workspaces: current.workspaces.map((workspace) =>
						workspace.id === workspaceId
							? {
									...workspace,
									portForwards: reconcileSavedPortForwardsForScope(
										(workspace.portForwards ?? []).map((portForward) => {
											if (portForward.id !== portForwardId) return portForward;
											const next = { ...portForward, ...updates };
											return {
												...next,
												...normalizeSavedPortForwardInput(next),
												updatedAt: now,
											};
										}),
										workspace.scope,
									),
									updatedAt: now,
								}
							: workspace,
					),
				};
			});
		},
		deleteSavedPortForward: (workspaceId, portForwardId) => {
			updateAndPersist((current) => {
				const now = new Date().toISOString();
				return {
					...current,
					workspaces: current.workspaces.map((workspace) =>
						workspace.id === workspaceId
							? {
									...workspace,
									portForwards: (workspace.portForwards ?? []).filter(
										(portForward) => portForward.id !== portForwardId,
									),
									updatedAt: now,
								}
							: workspace,
					),
				};
			});
		},
		togglePinnedResource: (workspaceId, resource) => {
			updateWorkspaceEntryPoints(workspaceId, (entryPoints, now) =>
				togglePinnedEntry(entryPoints, entryPointFromResource(resource, now)),
			);
		},
		recordRecentNamespace: (workspaceId, clusterContext, namespace) => {
			updateWorkspaceEntryPoints(workspaceId, (entryPoints, now) =>
				recordRecentEntry(
					entryPoints,
					entryPointFromNamespace(clusterContext, namespace, now),
				),
			);
		},
		recordRecentApplication: (workspaceId, clusterContext, name, namespace) => {
			updateWorkspaceEntryPoints(workspaceId, (entryPoints, now) =>
				recordRecentEntry(
					entryPoints,
					entryPointFromApplication(clusterContext, name, namespace, now),
				),
			);
		},
		recordRecentResource: (workspaceId, resource) => {
			updateWorkspaceEntryPoints(workspaceId, (entryPoints, now) =>
				recordRecentEntry(entryPoints, entryPointFromResource(resource, now)),
			);
		},
		reconcileEntryPoints: (workspaceId, resources, coverage) => {
			updateWorkspaceEntryPoints(workspaceId, (entryPoints) =>
				reconcileEntryPoints(entryPoints, resources, coverage),
			);
		},
		importWorkspaces: (preview, decisions) => {
			let result: WorkspaceImportResult = {
				workspaces: [],
				added: 0,
				replaced: 0,
				skipped: 0,
			};
			updateAndPersist((current) => {
				result = applyWorkspaceImport(current.workspaces, preview, decisions);
				return {
					workspaces: result.workspaces,
					selectedWorkspaceId: current.selectedWorkspaceId,
				};
			});
			return result;
		},
		deleteWorkspace: (id) => {
			updateAndPersist((current) => ({
				workspaces: current.workspaces.filter((workspace) => workspace.id !== id),
				selectedWorkspaceId:
					current.selectedWorkspaceId === id
						? null
						: current.selectedWorkspaceId,
			}));
		},
		openWorkspace: (id) => {
			state.update((current) => ({
				...current,
				selectedWorkspaceId: current.workspaces.some(
					(workspace) => workspace.id === id,
				)
					? id
					: null,
			}));
		},
		clearSelectedWorkspace: () => {
			state.update((current) => ({ ...current, selectedWorkspaceId: null }));
		},
	};
}

export const workspaceStore = createWorkspaceStore();

export function getWorkspaceSnapshot(): WorkspaceState {
	return get(workspaceStore);
}
