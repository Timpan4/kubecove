import { describe, expect, test } from "bun:test";
import { get } from "svelte/store";
import {
	createWorkspaceStore,
	readPersistedWorkspaces,
	writePersistedWorkspaces,
} from "../src/features/workspaces/workspaceStore";
import type { ResourceSummary } from "../src/lib/types";
import {
	createSavedPortForward,
	createWorkspaceRecord,
} from "../src/lib/workspace-model";

function makeStorage() {
	const values = new Map<string, string>();
	let writes = 0;
	return {
		getItem: (key: string) => values.get(key) ?? null,
		setItem: (key: string, value: string) => {
			writes += 1;
			values.set(key, value);
		},
		writeCount: () => writes,
	};
}

function persistedWorkspaces(storage: ReturnType<typeof makeStorage>) {
	return JSON.parse(storage.getItem("kubecove-workspaces") ?? "{}").state
		.workspaces;
}

function resource(name: string): ResourceSummary {
	return {
		cluster: "kind-dev",
		namespace: "default",
		kind: "Deployment",
		apiVersion: "apps/v1",
		name,
		age: "1h",
		health: "healthy",
	};
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

	test("create, edit, and delete keep workspace storage shape", () => {
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

	test("updates saved port-forward status in workspace storage", () => {
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

	test("adds and deletes saved port-forwards in workspace storage", () => {
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

	test("persists explicit namespace, application, and resource entry points", () => {
		const storage = makeStorage();
		const store = createWorkspaceStore(storage);
		const workspace = store.createWorkspace({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: ["default"],
		});

		store.recordRecentNamespace(workspace.id, "kind-dev", "default");
		store.recordRecentApplication(workspace.id, "kind-dev", "checkout", "argocd");
		store.recordRecentResource(workspace.id, resource("api"));

		expect(persistedWorkspaces(storage)[0].entryPoints.recent).toMatchObject([
			{ kind: "resource", name: "api", resourceKind: "Deployment" },
			{ kind: "app", name: "checkout", namespace: "argocd" },
			{ kind: "namespace", name: "default", namespace: "default" },
		]);
	});

	test("does not persist or update timestamps when reconciliation is unchanged", () => {
		const storage = makeStorage();
		const store = createWorkspaceStore(storage);
		const workspace = store.createWorkspace({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: ["default"],
		});
		store.togglePinnedResource(workspace.id, resource("api"));
		const before = get(store.workspaces)[0];
		const writesBefore = storage.writeCount();
		let notifications = 0;
		const unsubscribe = store.subscribe(() => {
			notifications += 1;
		});
		const notificationsBefore = notifications;

		store.reconcileEntryPoints(
			workspace.id,
			[resource("api")],
			[
				{
					clusterContext: "kind-dev",
					requests: [{ kind: "Deployment", namespace: "default" }],
				},
			],
		);

		unsubscribe();
		expect(notifications).toBe(notificationsBefore);
		expect(storage.writeCount()).toBe(writesBefore);
		expect(get(store.workspaces)[0]).toBe(before);
		expect(get(store.workspaces)[0].updatedAt).toBe(before.updatedAt);
	});

	test("prunes covered missing resources while retaining out-of-scope identities", () => {
		const storage = makeStorage();
		const store = createWorkspaceStore(storage);
		const workspace = store.createWorkspace({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: ["default"],
		});
		store.togglePinnedResource(workspace.id, resource("missing"));
		store.togglePinnedResource(
			workspace.id,
			{ ...resource("payments-api"), namespace: "payments" },
		);

		store.reconcileEntryPoints(
			workspace.id,
			[],
			[
				{
					clusterContext: "kind-dev",
					requests: [{ kind: "Deployment", namespace: "default" }],
				},
			],
		);

		expect(persistedWorkspaces(storage)[0].entryPoints.pinned).toMatchObject([
			{ name: "payments-api", namespace: "payments" },
		]);
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
