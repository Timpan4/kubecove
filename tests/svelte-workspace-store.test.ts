import { describe, expect, test } from "bun:test";
import { get } from "svelte/store";
import {
	createSavedPortForward,
	createWorkspaceRecord,
} from "../src/lib/workspace-model";
import {
	createWorkspaceStore,
	readPersistedWorkspaces,
	writePersistedWorkspaces,
} from "../src/features/workspaces/workspaceStore";

function makeStorage() {
	const values = new Map<string, string>();
	return {
		getItem: (key: string) => values.get(key) ?? null,
		setItem: (key: string, value: string) => values.set(key, value),
	};
}

function persistedWorkspaces(storage: ReturnType<typeof makeStorage>) {
	return JSON.parse(storage.getItem("kubecove-workspaces") ?? "{}").state
		.workspaces;
}

describe("svelte workspace store", () => {
	test("reads existing persisted workspaces", () => {
		const storage = makeStorage();
		const workspace = createWorkspaceRecord(
			{
				name: "Ops",
				clusterContext: "kind-dev",
				namespaces: ["default"],
			},
			"2026-06-16T12:00:00.000Z",
		);
		writePersistedWorkspaces([workspace], storage);

		const store = createWorkspaceStore(storage);

		expect(get(store.workspaces).map((item) => item.id)).toEqual([workspace.id]);
		expect(readPersistedWorkspaces(storage)).toEqual([workspace]);
	});

	test("create, edit, and delete keep React-readable storage shape", () => {
		const storage = makeStorage();
		const store = createWorkspaceStore(storage);

		const workspace = store.createWorkspace({
			name: "Ops",
			clusterContext: "kind-dev",
			clusterContexts: ["kind-prod"],
			namespaces: ["payments", "default"],
		});

		expect(persistedWorkspaces(storage)[0]).toMatchObject({
			id: workspace.id,
			name: "Ops",
			scope: {
				clusterContext: "kind-dev",
				namespaces: ["default", "payments"],
			},
			portForwards: [],
		});

		store.updateWorkspace(workspace.id, {
			name: "Ops 2",
			clusterContext: "kind-prod",
			namespaces: ["apps"],
		});

		expect(persistedWorkspaces(storage)[0]).toMatchObject({
			id: workspace.id,
			name: "Ops 2",
			scope: {
				clusterContext: "kind-prod",
				namespaces: ["apps"],
			},
		});

		store.deleteWorkspace(workspace.id);

		expect(persistedWorkspaces(storage)).toEqual([]);
	});

	test("updates saved port-forward status in React-readable storage", () => {
		const storage = makeStorage();
		const portForward = createSavedPortForward({
			clusterContext: "kind-dev",
			namespace: "default",
			serviceName: "api",
			servicePort: 8080,
		});
		const workspace = {
			...createWorkspaceRecord({
				name: "Ops",
				clusterContext: "kind-dev",
				namespaces: ["default"],
			}),
			portForwards: [portForward],
		};
		writePersistedWorkspaces([workspace], storage);
		const store = createWorkspaceStore(storage);
		const portForwardId = workspace.portForwards[0].id;

		store.updateSavedPortForward(workspace.id, portForwardId, {
			lastStatus: "listening",
			lastStartedAt: "2026-06-18T09:00:00.000Z",
		});

		expect(persistedWorkspaces(storage)[0].portForwards[0]).toMatchObject({
			id: portForwardId,
			lastStatus: "listening",
			lastStartedAt: "2026-06-18T09:00:00.000Z",
			serviceName: "api",
		});
	});

	test("adds and deletes saved port-forwards in React-readable storage", () => {
		const storage = makeStorage();
		const store = createWorkspaceStore(storage);
		const workspace = store.createWorkspace({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: ["default"],
		});

		store.saveSavedPortForward(workspace.id, {
			clusterContext: "kind-dev",
			namespace: "default",
			serviceName: "api",
			servicePort: 8080,
			label: "API",
		});

		const saved = persistedWorkspaces(storage)[0].portForwards[0];
		expect(saved).toMatchObject({
			clusterContext: "kind-dev",
			namespace: "default",
			serviceName: "api",
			servicePort: 8080,
			label: "API",
			lastStatus: "idle",
		});

		store.deleteSavedPortForward(workspace.id, saved.id);

		expect(persistedWorkspaces(storage)[0].portForwards).toEqual([]);
	});

	test("deleting selected workspace clears selected placeholder", () => {
		const store = createWorkspaceStore(makeStorage());
		const workspace = store.createWorkspace({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: [],
		});

		expect(get(store.selectedWorkspace)?.id).toBe(workspace.id);

		store.deleteWorkspace(workspace.id);

		expect(get(store.selectedWorkspace)).toBeNull();
		expect(get(store.selectedWorkspaceId)).toBeNull();
	});

	test("openWorkspace selects existing workspace and clears missing handoff ids", () => {
		const store = createWorkspaceStore(makeStorage());
		const first = store.createWorkspace({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: [],
		});
		const second = store.createWorkspace({
			name: "Prod",
			clusterContext: "kind-prod",
			namespaces: ["payments"],
		});

		store.openWorkspace(first.id);

		expect(get(store.selectedWorkspaceId)).toBe(first.id);
		expect(get(store.selectedWorkspace)?.name).toBe("Ops");

		store.openWorkspace("missing-workspace");

		expect(get(store.selectedWorkspaceId)).toBeNull();
		expect(get(store.selectedWorkspace)).toBeNull();
		expect(get(store.workspaces).map((workspace) => workspace.id)).toEqual([
			second.id,
			first.id,
		]);
	});
});
