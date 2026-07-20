import { describe, expect, test } from "bun:test";
import { cockpitItems } from "../src/features/rbac/cockpitModel";
import {
	buildRbacStats,
	buildRbacTable,
	rbacWarningSummary,
	selectedRbacView,
} from "../src/features/rbac/surfaceModel";
import type { RbacInspectionSummary } from "../src/lib/types";

const summary: RbacInspectionSummary = {
	cluster: "kind-dev",
	warnings: [],
	coverage: [],
	serviceAccounts: [
		{
			cluster: "kind-dev",
			name: "builder",
			namespace: "default",
			age: "1d",
			secretsCount: 1,
			imagePullSecretsCount: 0,
			automountToken: true,
			risks: [{ level: "medium", label: "automount-token", reason: "token mounted" }],
		},
	],
	roles: [
		{
			cluster: "kind-dev",
			kind: "Role",
			name: "editor",
			namespace: "default",
			age: "1d",
			rulesCount: 2,
			risks: [],
			rules: [],
		},
	],
	clusterRoles: [
		{
			cluster: "kind-dev",
			kind: "ClusterRole",
			name: "admin",
			age: "1d",
			rulesCount: 7,
			risks: [{ level: "high", label: "wildcard", reason: "wildcard verbs" }],
			rules: [],
		},
	],
	roleBindings: [
		{
			cluster: "kind-dev",
			kind: "RoleBinding",
			name: "bind-edit",
			namespace: "default",
			age: "1d",
			roleRefKind: "Role",
			roleRefName: "editor",
			subjects: [{ kind: "ServiceAccount", namespace: "default", name: "builder" }],
			risks: [],
		},
	],
	clusterRoleBindings: [],
	namespaceAccess: [
		{
			cluster: "kind-dev",
			namespace: "default",
			serviceAccounts: 1,
			roles: 1,
			roleBindings: 1,
			boundSubjects: [{ kind: "ServiceAccount", namespace: "default", name: "builder" }],
			risks: [],
		},
	],
};

describe("svelte RBAC surface model", () => {
	test("selects RBAC view from sidebar node", () => {
		expect(
			selectedRbacView({
				type: "kind",
				section: "rbac",
				kind: "Service Accounts",
			}),
		).toBe("Service Accounts");
		expect(selectedRbacView({ type: "section", section: "rbac" })).toBe(
			"Service Accounts",
		);
	});

	test("builds selected RBAC table rows", () => {
		expect(buildRbacTable(summary, "Cluster Roles")).toMatchObject({
			headers: ["Kind", "Name", "Rules", "Risks"],
			rows: [["ClusterRole", "admin", "7", "high: wildcard"]],
		});
		expect(buildRbacTable(summary, "Bindings").rows[0]).toEqual([
			"RoleBinding",
			"bind-edit",
			"default",
			"Role/editor",
			"ServiceAccount/default/builder",
			"-",
		]);
	});

	test("builds RBAC stats with separate risk flags and flagged objects", () => {
		expect(buildRbacStats(summary)).toEqual([
			["Service accounts", 1],
			["Roles", 1],
			["Cluster roles", 1],
			["Bindings", 1],
			["Risk flags", 2],
			["Flagged objects", 2],
		]);
	});

	test("summarizes RBAC warnings", () => {
		expect(rbacWarningSummary(["a", "b", "c", "d", "e"])).toBe(
			"a b c 2 more warnings.",
		);
	});

	test("inherits referenced role risk into bindings and service accounts", () => {
		const inherited: RbacInspectionSummary = {
			...summary,
			coverage: [
				{ family: "serviceAccounts", status: "complete", requestMode: "allNamespaces", count: 1 },
				{ family: "roles", status: "complete", requestMode: "allNamespaces", count: 1 },
				{ family: "clusterRoles", status: "complete", requestMode: "cluster", count: 1 },
				{ family: "roleBindings", status: "complete", requestMode: "allNamespaces", count: 1 },
				{ family: "clusterRoleBindings", status: "complete", requestMode: "cluster", count: 0 },
			],
			roles: [{
				...summary.roles[0],
				risks: [{ level: "high", label: "secrets-access", reason: "reads Secrets" }],
			}],
		};

		expect(cockpitItems(inherited, "Bindings")[0]?.risks).toContainEqual({
			level: "high",
			label: "secrets-access",
			reason: "reads Secrets",
		});
		expect(cockpitItems(inherited, "Service Accounts")[0]?.risks).toContainEqual({
			level: "high",
			label: "secrets-access",
			reason: "reads Secrets",
		});
		expect(cockpitItems(inherited, "Service Accounts")[0]?.kind).toBe(
			"ServiceAccount",
		);
	});
});
