import { describe, expect, test } from "bun:test";
import { identityDefaults, observedPermissions } from "../src/features/rbac/observedPermissions";
import type { RbacInspectionSummary } from "../src/lib/types";

const base = (): RbacInspectionSummary => ({
	cluster: "kind", refreshedAt: "2026-07-18T10:00:00Z", warnings: [],
	coverage: ["roles", "clusterRoles", "roleBindings", "clusterRoleBindings"].map((family) => ({ family, status: "complete", requestMode: "cluster", count: 1 })),
	serviceAccounts: [],
	roles: [{ cluster: "kind", kind: "Role", name: "reader", namespace: "team", age: "1d", rulesCount: 1, risks: [], rules: [{ verbs: ["get"], apiGroups: [""], resources: ["pods"], resourceNames: ["named"], nonResourceUrls: [], risks: [] }] }],
	clusterRoles: [{ cluster: "kind", kind: "ClusterRole", name: "viewer", age: "1d", rulesCount: 1, risks: [], rules: [{ verbs: ["list"], apiGroups: [""], resources: ["nodes"], resourceNames: [], nonResourceUrls: ["/healthz"], risks: [] }] }],
	roleBindings: [{ cluster: "kind", kind: "RoleBinding", name: "read", namespace: "team", age: "1d", roleRefKind: "Role", roleRefName: "reader", subjects: [{ kind: "ServiceAccount", name: "api", namespace: "team" }], risks: [] }],
	clusterRoleBindings: [{ cluster: "kind", kind: "ClusterRoleBinding", name: "view", age: "1d", roleRefKind: "ClusterRole", roleRefName: "viewer", subjects: [{ kind: "User", name: "alex" }, { kind: "Group", name: "ops" }, { kind: "Group", name: "system:serviceaccounts" }], risks: [] }],
	namespaceAccess: [],
});

describe("observed RBAC permissions", () => {
	test("keeps RoleBinding grants namespaced and ClusterRoleBinding grants cluster scoped", () => {
		const inspection = base();
		expect(
			observedPermissions(inspection, {
				kind: "serviceAccount",
				name: "api",
				namespace: "team",
			}).grants.find((grant) => grant.scope === "namespace"),
		).toMatchObject({
			scope: "namespace",
			namespace: "team",
			resourceNames: ["named"],
			bindings: ["RoleBinding/team/read"],
		});
		expect(observedPermissions(inspection, { kind: "user", name: "alex" }).grants).toMatchObject([{ scope: "cluster", nonResourceUrls: ["/healthz"], roles: ["ClusterRole/_/viewer"] }]);
	});
	test("keeps available grants when coverage is incomplete and reports unknown", () => {
		const inspection = base();
		expect(observedPermissions(inspection, { kind: "group", name: "ops" }).grants).toHaveLength(1);
		inspection.coverage[0].status = "partial";
		expect(observedPermissions(inspection, { kind: "group", name: "ops" })).toMatchObject({ unknown: true, grants: [{ scope: "cluster" }] });
	});
	test("matches submitted user groups and automatic ServiceAccount groups", () => {
		const inspection = base();
		expect(
			observedPermissions(inspection, {
				kind: "user",
				name: "mira",
				groups: ["ops"],
			}).grants,
		).toHaveLength(1);
		expect(
			observedPermissions(inspection, {
				kind: "serviceAccount",
				name: "unbound",
				namespace: "team",
			}).grants.some((grant) => grant.scope === "cluster"),
		).toBe(true);
	});
	test("uses safe user defaults", () => {
		expect(identityDefaults("user", "alex")).toEqual(["system:authenticated"]);
		expect(identityDefaults("user", "system:anonymous")).toEqual(["system:unauthenticated"]);
	});
});
