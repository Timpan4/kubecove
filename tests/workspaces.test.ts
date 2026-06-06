import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
	buildWorkspaceCompareEntries,
	buildWorkspaceCompareSummaries,
	buildWorkspaceHealthSummary,
	computeRestoreStatus,
	createSavedPortForward,
	createWorkspaceRecord,
	createWorkspaceScope,
	summarizeWorkspaceScope,
	useWorkspaceStore,
	workspaceScopeContexts,
} from "../src/lib/workspaces";
import {
	buildWorkspaceFetchKeys,
	buildWorkspaceFetchPlans,
} from "../src/features/workspaces/query";
import type {
	ClusterContext,
	ResourceKindSelection,
	ResourceSummary,
} from "../src/lib/types";

const clusterContexts: ClusterContext[] = [
	{ name: "kind-dev", isCurrent: true },
	{ name: "kind-prod", isCurrent: false },
];

describe("workspace helpers", () => {
	test("creates sorted local workspace records without kubeconfig data", () => {
		const workspace = createWorkspaceRecord(
			{
				name: " Dev ",
				clusterContext: "kind-dev",
				namespaces: ["payments", "default"],
			},
			"2026-05-16T12:00:00.000Z",
		);

		expect(workspace.name).toBe("Dev");
		expect(workspace.scope.clusterContext).toBe("kind-dev");
		expect(workspace.scope.namespaces).toEqual(["default", "payments"]);
		expect(workspace.scope.kinds).toContain("Pod");
		expect(JSON.stringify(workspace)).not.toContain("certificate");
		expect(JSON.stringify(workspace)).not.toContain("token");
		expect(workspace.portForwards).toEqual([]);
	});

	test("creates Service-only saved port-forward presets without session IDs", () => {
		const saved = createSavedPortForward(
			{
				clusterContext: " kind-dev ",
				namespace: " payments ",
				serviceName: " api ",
				servicePort: 8080,
				localPort: 18080,
				label: " API ",
			},
			"2026-05-31T12:00:00.000Z",
		);

		expect(saved).toMatchObject({
			clusterContext: "kind-dev",
			namespace: "payments",
			serviceName: "api",
			servicePort: 8080,
			localPort: 18080,
			label: "API",
			lastStatus: "idle",
			createdAt: "2026-05-31T12:00:00.000Z",
			updatedAt: "2026-05-31T12:00:00.000Z",
		});
		expect(JSON.stringify(saved)).not.toContain("session");
		expect(JSON.stringify(saved)).not.toContain("podName");
	});

	test("manages saved Service forwards in workspace state", () => {
		useWorkspaceStore.setState({ workspaces: [], activeWorkspaceId: null });
		const store = useWorkspaceStore.getState();
		const workspace = store.createWorkspace({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: ["payments"],
		});

		const saved = useWorkspaceStore.getState().savePortForward(workspace.id, {
			clusterContext: "kind-dev",
			namespace: "payments",
			serviceName: "api",
			servicePort: 8080,
		});
		expect(
			useWorkspaceStore.getState().workspaces[0].portForwards.map(
				(portForward) => portForward.id,
			),
		).toEqual([saved.id]);

		useWorkspaceStore.getState().updateSavedPortForward(workspace.id, saved.id, {
			label: "Payments API",
			localPort: 18080,
			lastStatus: "error",
			lastError: "local port 18080 is already in use",
		});
		expect(useWorkspaceStore.getState().workspaces[0].portForwards[0]).toMatchObject({
			label: "Payments API",
			localPort: 18080,
			lastStatus: "error",
			lastError: "local port 18080 is already in use",
		});

		useWorkspaceStore.getState().updateSavedPortForward(workspace.id, saved.id, {
			localPort: undefined,
		});
		expect(
			useWorkspaceStore.getState().workspaces[0].portForwards[0].localPort,
		).toBeUndefined();

		useWorkspaceStore
			.getState()
			.deleteSavedPortForward(workspace.id, saved.id);
		expect(useWorkspaceStore.getState().workspaces[0].portForwards).toEqual([]);
	});

	test("reconciles saved forwards when workspace scope changes", () => {
		useWorkspaceStore.setState({ workspaces: [], activeWorkspaceId: null });
		const workspace = useWorkspaceStore.getState().createWorkspace({
			name: "Ops",
			clusterContext: "kind-dev",
			namespaces: ["payments"],
		});
		const saved = useWorkspaceStore.getState().savePortForward(workspace.id, {
			clusterContext: "kind-dev",
			namespace: "payments",
			serviceName: "api",
			servicePort: 8080,
		});

		useWorkspaceStore.getState().updateWorkspace(workspace.id, {
			scope: createWorkspaceScope({
				name: "Ops",
				clusterContext: "kind-prod",
				namespaces: ["payments"],
			}),
		});

		expect(useWorkspaceStore.getState().workspaces[0].portForwards).toEqual([]);

		useWorkspaceStore.getState().updateWorkspace(workspace.id, {
			portForwards: [saved],
			scope: createWorkspaceScope({
				name: "Ops",
				clusterContext: "kind-dev",
				namespaces: [],
			}),
		});

		expect(useWorkspaceStore.getState().workspaces[0].portForwards).toEqual([
			saved,
		]);

		useWorkspaceStore.getState().updateWorkspace(workspace.id, {
			scope: createWorkspaceScope({
				name: "Ops",
				clusterContext: "kind-dev",
				namespaces: ["other"],
			}),
		});

		expect(useWorkspaceStore.getState().workspaces[0].portForwards).toEqual([
			saved,
		]);
	});

	test("stores cluster groups as local scope metadata without secrets", () => {
		const workspace = createWorkspaceRecord(
			{
				name: "Ops",
				clusterContext: "kind-dev",
				clusterContexts: ["kind-prod", "kind-dev"],
				clusterGroupName: "Ops group",
				namespaces: ["payments", "default"],
			},
			"2026-05-16T12:00:00.000Z",
		);

		expect(workspace.scope.clusterGroup).toEqual({
			id: "cluster-group:kind-dev|kind-prod",
			name: "Ops group",
			members: ["kind-dev", "kind-prod"],
		});
		expect(workspaceScopeContexts(workspace.scope)).toEqual([
			"kind-dev",
			"kind-prod",
		]);
		expect(summarizeWorkspaceScope(workspace.scope)).toBe(
			"Ops group (2) / default, payments / Pod, Deployment +4",
		);
		expect(JSON.stringify(workspace)).not.toContain("kubeconfig");
		expect(JSON.stringify(workspace)).not.toContain("client-key");
	});

	test("reports missing restore scopes", () => {
		const unknownKind = "NotARealKind" as unknown as ResourceKindSelection;
		const workspace = createWorkspaceRecord(
			{
				name: "Ops",
				clusterContext: "kind-dev",
				namespaces: ["default", "missing"],
				kinds: ["Pod", unknownKind],
			},
			"2026-05-16T12:00:00.000Z",
		);

		const status = computeRestoreStatus(
			workspace,
			clusterContexts,
			["default"],
			[],
		);

		expect(status.clusterAvailable).toBe(true);
		expect(status.missingClusterContexts).toEqual([]);
		expect(status.missingNamespaces).toEqual(["missing"]);
		expect(status.missingKinds).toEqual(["NotARealKind"]);
	});

	test("reports unavailable saved contexts in a cluster group", () => {
		const workspace = createWorkspaceRecord(
			{
				name: "Ops",
				clusterContext: "kind-dev",
				clusterContexts: ["missing-context"],
				namespaces: [],
			},
			"2026-05-16T12:00:00.000Z",
		);

		const status = computeRestoreStatus(workspace, clusterContexts, [], []);

		expect(status.clusterAvailable).toBe(true);
		expect(status.missingClusterContexts).toEqual(["missing-context"]);
	});

	test("summarizes scope and health", () => {
		const workspace = createWorkspaceRecord(
			{
				name: "Ops",
				clusterContext: "kind-dev",
				namespaces: ["default", "payments", "web"],
				kinds: ["Pod", "Deployment", "Service"],
			},
			"2026-05-16T12:00:00.000Z",
		);
		const rows = [
			resource({ status: "Running", ready: "true" }),
			resource({ status: "Pending" }),
			resource({ status: "Failed" }),
			resource({ status: "Running", restarts: 2 }),
		];

		expect(summarizeWorkspaceScope(workspace.scope)).toBe(
			"kind-dev / default, payments +1 / Pod, Deployment +1",
		);
		expect(buildWorkspaceHealthSummary(rows)).toEqual({
			total: 4,
			healthy: 1,
			attention: 2,
			degraded: 1,
			restarted: 1,
		});
	});

	test("builds context and namespace compare summaries from resource health", () => {
		const workspace = createWorkspaceRecord(
			{
				name: "Ops",
				clusterContext: "kind-dev",
				clusterContexts: ["kind-prod"],
				namespaces: ["default", "payments"],
				kinds: ["Pod"],
			},
			"2026-05-16T12:00:00.000Z",
		);
		const rows = [
			resource({ cluster: "kind-dev", namespace: "default", status: "Running", ready: "true" }),
			resource({ cluster: "kind-prod", namespace: "default", status: "Failed" }),
			resource({ cluster: "kind-dev", namespace: "payments", status: "Pending" }),
		];
		const entries = buildWorkspaceCompareEntries(workspace.scope);
		const summaries = buildWorkspaceCompareSummaries(entries, rows);

		expect(entries.map((entry) => entry.kind)).toEqual([
			"contexts",
			"namespaces",
		]);
		expect(summaries[0].left.total).toBe(2);
		expect(summaries[0].right.degraded).toBe(1);
		expect(summaries[1].left.total).toBe(2);
		expect(summaries[1].right.attention).toBe(1);
	});

	test("preserves all-namespace workspace resource fetches", () => {
		const workspace = createWorkspaceRecord(
			{
				name: "Ops",
				clusterContext: "kind-dev",
				namespaces: [],
				kinds: ["Pod", "Node"],
			},
			"2026-05-16T12:00:00.000Z",
		);

		expect(buildWorkspaceFetchKeys(workspace.scope, [])).toEqual([
			{ kind: "Pod" },
			{ kind: "Node" },
		]);
	});

	test("does not widen namespace-scoped workspace resources to cluster scope", () => {
		const workspace = createWorkspaceRecord(
			{
				name: "Ops",
				clusterContext: "kind-dev",
				namespaces: ["missing"],
				kinds: ["Pod", "Node"],
			},
			"2026-05-16T12:00:00.000Z",
		);

		expect(buildWorkspaceFetchKeys(workspace.scope, [])).toEqual([
			{ kind: "Node" },
		]);
	});

	test("keeps namespace requests per context for cluster groups", () => {
		const workspace = createWorkspaceRecord(
			{
				name: "Ops",
				clusterContext: "kind-dev",
				clusterContexts: ["kind-prod"],
				namespaces: ["missing"],
				kinds: ["Pod", "Node"],
			},
			"2026-05-16T12:00:00.000Z",
		);

		expect(buildWorkspaceFetchKeys(workspace.scope, [])).toEqual([
			{ kind: "Node" },
		]);
		expect(buildWorkspaceFetchPlans(workspace.scope, [])).toEqual([
			{
				clusterContext: "kind-dev",
				requests: [{ kind: "Pod", namespace: "missing" }, { kind: "Node" }],
			},
			{
				clusterContext: "kind-prod",
				requests: [{ kind: "Pod", namespace: "missing" }, { kind: "Node" }],
			},
		]);
	});

	test("uses an explicit height for the workspace namespace scroll area", () => {
		const source = readFileSync(
			"src/features/workspaces/WorkspaceLauncher.tsx",
			"utf8",
		);

		expect(source).toContain(
			'ScrollArea className="h-52 rounded-md border bg-background/40"',
		);
		expect(source).not.toContain(
			'ScrollArea className="max-h-52 rounded-md border bg-background/40"',
		);
	});

	test("allows workspace saves when namespace listing fails", () => {
		const source = readFileSync(
			"src/features/workspaces/WorkspaceLauncher.tsx",
			"utf8",
		);

		expect(source).not.toContain("namespaceScopeUnavailable");
		expect(source).toContain(
			"You can still save an all-namespace workspace",
		);
	});

	test("keeps overview Resources primary and visible in wrapping actions", () => {
		const source = readFileSync(
			"src/features/workspaces/WorkspaceOverview.tsx",
			"utf8",
		);
		const headerActionsStart = source.indexOf(
			'className="flex flex-wrap justify-end gap-2"',
		);
		const headerActionsEnd = source.indexOf("</div>", headerActionsStart);
		const headerActions = source.slice(headerActionsStart, headerActionsEnd);

		expect(headerActionsStart).toBeGreaterThanOrEqual(0);
		expect(headerActions.indexOf("Resources")).toBeLessThan(
			headerActions.indexOf("Workspaces"),
		);
		expect(headerActions.indexOf("Resources")).toBeLessThan(
			headerActions.indexOf("Port Forwards"),
		);
		expect(headerActions.indexOf("Resources")).toBeLessThan(
			headerActions.indexOf("Incidents"),
		);
	});
});

function resource(overrides: Partial<ResourceSummary>): ResourceSummary {
	return {
		kind: "Pod",
		cluster: "kind-dev",
		name: "nginx",
		namespace: "default",
		age: "1m",
		...overrides,
	};
}
