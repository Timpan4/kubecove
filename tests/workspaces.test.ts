import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
	buildWorkspaceFetchKeys,
	buildWorkspaceFetchPlans,
	WorkspaceResourceLoadError,
} from "../src/features/workspaces/query";
import type {
	ClusterContext,
	ResourceKindSelection,
	ResourceSummary,
} from "../src/lib/types";
import {
	buildWorkspaceCompareEntries,
	buildWorkspaceCompareSummaries,
	buildWorkspaceHealthSummary,
	computeRestoreStatus,
	createSavedPortForward,
	createWorkspaceRecord,
	summarizeWorkspaceScope,
	workspaceScopeContexts,
} from "../src/lib/workspaces";

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
			"Ops group (2) / default, payments / Pod, Deployment +7",
		);
		expect(JSON.stringify(workspace)).not.toContain("kubeconfig");
		expect(JSON.stringify(workspace)).not.toContain("client-key");
	});

	test("reports missing restore scopes", () => {
		const unknownKind: ResourceKindSelection = "NotARealKind";
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
			resource({ status: "Running", ready: "true", health: "healthy" }),
			resource({ status: "Pending", health: "attention" }),
			resource({ status: "Failed", health: "degraded" }),
			resource({ status: "Running", health: "restarted", restarts: 2 }),
		];

		expect(summarizeWorkspaceScope(workspace.scope)).toBe(
			"kind-dev / default, payments +1 / Pod, Deployment +1",
		);
		expect(buildWorkspaceHealthSummary(rows)).toEqual({
			total: 4,
			healthy: 2,
			attention: 1,
			degraded: 1,
			unknown: 0,
			notEvaluated: 0,
			restartEvidence: 1,
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
			resource({ cluster: "kind-dev", namespace: "default", status: "Running", ready: "true", health: "healthy" }),
			resource({ cluster: "kind-prod", namespace: "default", status: "Failed", health: "degraded" }),
			resource({ cluster: "kind-dev", namespace: "payments", status: "Pending", health: "attention" }),
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

	test("workspace resource failures retain context, operation, and technical detail", () => {
		const error = new WorkspaceResourceLoadError([
			{
				clusterContext: "kind-prod",
				reason: { kind: "cluster", message: "ServiceError: client error (Connect)" },
			},
		]);

		expect(error.clusterContexts).toEqual(["kind-prod"]);
		expect(error.kind).toBe("networkTransient");
		expect(error.message).toBe(
			'Context "kind-prod", operation "resource discovery": ServiceError: client error (Connect)',
		);
	});

	test("workspace resource failures keep mixed context guidance neutral", () => {
		const error = new WorkspaceResourceLoadError([
			{
				clusterContext: "kind-dev",
				reason: { kind: "cluster", message: "Unauthorized: authentication required" },
			},
			{
				clusterContext: "kind-prod",
				reason: { kind: "cluster", message: "ServiceError: client error (Connect)" },
			},
		]);

		expect(error.kind).toBe("mixedWorkspaceConnection");
		expect(error.failureBuckets).toEqual(["authentication", "networkTransient"]);
	});

	test("unrelated mixed workspace failures use generic guidance", () => {
		const error = new WorkspaceResourceLoadError([
			{
				clusterContext: "kind-dev",
				reason: { kind: "forbidden", message: "pods is forbidden" },
			},
			{
				clusterContext: "kind-prod",
				reason: { kind: "validation", message: "namespace is required" },
			},
		]);

		expect(error.kind).toBe("unknown");
		expect(error.failureBuckets).toEqual(["forbiddenRbac", "validation"]);
	});

	test("uses an explicit height for the workspace namespace scroll area", () => {
		const source = readFileSync(
			"src/features/workspaces/WorkspaceLauncher.svelte",
			"utf8",
		);

		expect(source).toContain(
			'ScrollArea class="h-52 rounded-md border bg-background/40"',
		);
		expect(source).not.toContain(
			'ScrollArea class="max-h-52 rounded-md border bg-background/40"',
		);
	});

	test("keeps overview Resources primary and visible in wrapping actions", () => {
		const source = readFileSync(
			"src/features/workspaces/WorkspaceOverview.svelte",
			"utf8",
		);
		const headerActionsStart = source.indexOf("<CardTitle>Operations</CardTitle>");
		const headerActionsEnd = source.indexOf("</Card>", headerActionsStart);
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
	const state = overrides.health === "degraded"
		? "degraded"
		: overrides.health === "attention"
			? "needsAttention"
		: overrides.health === "unknown" || overrides.health === undefined
				? "unknown"
				: "healthy";
	return {
		kind: "Pod",
		cluster: "kind-dev",
		name: "nginx",
		namespace: "default",
		age: "1m",
		health: "unknown",
		healthAssessment: {
			state,
			completeness: "complete",
			winningSources: ["kubernetes"],
			reasons: [`Test Kubernetes state is ${state}`],
			evidence: [{ source: "kubernetes", raw: state, state, current: true, reason: `Test Kubernetes state is ${state}` }],
		},
		...overrides,
	};
}
