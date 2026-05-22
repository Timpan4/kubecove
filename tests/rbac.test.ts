import { describe, expect, test } from "bun:test";
import {
	collectInspectionRisks,
	highestRisk,
	riskSummaryLabel,
	subjectLabel,
} from "../src/features/rbac/risk";
import type { RbacInspectionSummary, RbacRiskIndicator } from "../src/lib/types";

const medium: RbacRiskIndicator = {
	level: "medium",
	label: "RBAC write access",
	reason: "Rule can change RBAC policy resources.",
};
const high: RbacRiskIndicator = {
	level: "high",
	label: "Secrets access",
	reason: "Rule can read or change Secret resources.",
};

describe("RBAC risk helpers", () => {
	test("chooses the highest risk by severity", () => {
		expect(highestRisk([medium, high])).toEqual(high);
		expect(highestRisk([])).toBeNull();
	});

	test("builds factual compact risk labels", () => {
		expect(riskSummaryLabel([])).toBe("No flags");
		expect(riskSummaryLabel([medium, high])).toBe("HIGH: 2 flags");
	});

	test("formats subjects with namespace when present", () => {
		expect(subjectLabel({ kind: "ServiceAccount", namespace: "payments", name: "api" })).toBe(
			"ServiceAccount:payments/api",
		);
		expect(subjectLabel({ kind: "User", name: "alice" })).toBe("User:alice");
	});

	test("deduplicates inspection risks from all RBAC object groups", () => {
		const inspection: RbacInspectionSummary = {
			cluster: "kind-dev",
			serviceAccounts: [],
			roles: [{ cluster: "kind-dev", kind: "Role", name: "reader", age: "1m", rulesCount: 1, risks: [medium], rules: [] }],
			clusterRoles: [{ cluster: "kind-dev", kind: "ClusterRole", name: "reader", age: "1m", rulesCount: 1, risks: [medium], rules: [] }],
			roleBindings: [],
			clusterRoleBindings: [{ cluster: "kind-dev", kind: "ClusterRoleBinding", name: "admin", age: "1m", roleRefKind: "ClusterRole", roleRefName: "cluster-admin", subjects: [], risks: [high] }],
			namespaceAccess: [],
		};

		expect(collectInspectionRisks(inspection)).toEqual([medium, high]);
	});
});
