import { describe, expect, test } from "bun:test";
import {
	buildWorkspaceHealthSummary,
	computeRestoreStatus,
	createWorkspaceRecord,
	summarizeWorkspaceScope,
} from "../src/lib/workspaces";
import { buildWorkspaceFetchKeys } from "../src/features/workspaces/query";
import type {
	ClusterContext,
	ResourceKindSelection,
	ResourceSummary,
} from "../src/lib/types";

const clusterContexts: ClusterContext[] = [
	{ name: "kind-dev", isCurrent: true },
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
		expect(status.missingNamespaces).toEqual(["missing"]);
		expect(status.missingKinds).toEqual(["NotARealKind"]);
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
			healthy: 2,
			attention: 1,
			degraded: 1,
			restarted: 1,
		});
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
