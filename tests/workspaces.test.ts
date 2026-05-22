import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
	buildWorkspaceCompareEntries,
	buildWorkspaceCompareSummaries,
	buildWorkspaceHealthSummary,
	computeRestoreStatus,
	createWorkspaceRecord,
	summarizeWorkspaceScope,
	workspaceScopeContexts,
} from "../src/lib/workspaces";
import { buildWorkspaceFetchKeys } from "../src/features/workspaces/query";
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
