import { create } from "zustand";
import { persist } from "zustand/middleware";
import {
	createSavedPortForward,
	createWorkspaceRecord,
	normalizeSavedPortForwardInput,
	reconcileSavedPortForwardsForScope,
	type CreateWorkspaceInput,
	type SavedPortForward,
	type SavedPortForwardUpdates,
	type SavedWorkspace,
	type SavePortForwardInput,
} from "./workspace-model";

export * from "./workspace-model";

type WorkspaceUpdates = Partial<
	Pick<SavedWorkspace, "name" | "scope" | "shortcuts" | "portForwards">
>;

interface WorkspaceStore {
	workspaces: SavedWorkspace[];
	activeWorkspaceId: string | null;
	createWorkspace: (input: CreateWorkspaceInput) => SavedWorkspace;
	updateWorkspace: (id: string, updates: WorkspaceUpdates) => void;
	savePortForward: (
		workspaceId: string,
		input: SavePortForwardInput,
	) => SavedPortForward;
	updateSavedPortForward: (
		workspaceId: string,
		portForwardId: string,
		updates: SavedPortForwardUpdates,
	) => void;
	deleteSavedPortForward: (
		workspaceId: string,
		portForwardId: string,
	) => void;
	deleteWorkspace: (id: string) => void;
	setActiveWorkspace: (id: string | null) => void;
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
					workspaces: state.workspaces.map((workspace) => {
						if (workspace.id !== id) return workspace;
						const scope = updates.scope ?? workspace.scope;
						return {
							...workspace,
							...updates,
							portForwards: reconcileSavedPortForwardsForScope(
								updates.portForwards ?? workspace.portForwards ?? [],
								scope,
							),
							updatedAt: new Date().toISOString(),
						};
					}),
				})),
			savePortForward: (workspaceId, input) => {
				const savedPortForward = createSavedPortForward(input);
				set((state) => ({
					workspaces: state.workspaces.map((workspace) =>
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
				return savedPortForward;
			},
			updateSavedPortForward: (workspaceId, portForwardId, updates) =>
				set((state) => {
					const now = new Date().toISOString();
					return {
						workspaces: state.workspaces.map((workspace) =>
							workspace.id === workspaceId
								? {
										...workspace,
										portForwards: reconcileSavedPortForwardsForScope(
											(workspace.portForwards ?? []).map((portForward) => {
												if (portForward.id !== portForwardId) {
													return portForward;
												}
												const next = { ...portForward, ...updates };
												return {
													...portForward,
													...normalizeSavedPortForwardInput(next),
													lastStartedAt:
														"lastStartedAt" in updates
															? updates.lastStartedAt
															: portForward.lastStartedAt,
													lastStatus:
														"lastStatus" in updates
															? updates.lastStatus
															: portForward.lastStatus,
													lastError:
														"lastError" in updates
															? updates.lastError
															: portForward.lastError,
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
				}),
			deleteSavedPortForward: (workspaceId, portForwardId) =>
				set((state) => {
					const now = new Date().toISOString();
					return {
						workspaces: state.workspaces.map((workspace) =>
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
				}),
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
