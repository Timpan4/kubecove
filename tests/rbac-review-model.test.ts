import { describe, expect, test } from "bun:test";
import { buildRbacBindingIndex, cockpitItems } from "../src/features/rbac/cockpitModel";
import {
	buildRbacEvidenceFingerprint,
	type RbacReviewRecord,
	rbacReviewSubjectLabels,
	removeRbacReviewRecord,
	reviewStatus,
	upsertRbacReviewRecord,
} from "../src/features/rbac/reviewModel";
import type { RbacInspectionSummary } from "../src/lib/types";

function inspection(cluster = "kind-dev"): RbacInspectionSummary {
	return {
		cluster,
		warnings: [],
		coverage: [],
		serviceAccounts: [{ cluster, name: "api", namespace: "team", age: "1d", secretsCount: 0, imagePullSecretsCount: 0, risks: [] }],
		roles: [{ cluster, kind: "Role", name: "reader", namespace: "team", age: "1d", rulesCount: 1, risks: [], rules: [{ verbs: ["get"], apiGroups: [""], resources: ["pods"], resourceNames: [], nonResourceUrls: [], risks: [] }] }],
		clusterRoles: [],
		roleBindings: [{ cluster, kind: "RoleBinding", name: "read", namespace: "team", age: "1d", roleRefKind: "Role", roleRefName: "reader", subjects: [{ kind: "ServiceAccount", name: "api", namespace: "team" }], risks: [] }],
		clusterRoleBindings: [],
		namespaceAccess: [{ cluster, namespace: "team", serviceAccounts: 1, roles: 1, roleBindings: 1, boundSubjects: [], risks: [] }],
	};
}

function roleEntry(data: RbacInspectionSummary) {
	const entry = cockpitItems(data, "Roles")[0];
	if (!entry) throw new Error("expected role entry");
	return entry;
}

function firstBinding(data: RbacInspectionSummary) {
	const binding = data.roleBindings[0];
	if (!binding) throw new Error("expected role binding");
	return binding;
}

function review(data: RbacInspectionSummary): RbacReviewRecord {
	const entry = roleEntry(data);
	const evidenceFingerprint = buildRbacEvidenceFingerprint(data, entry);
	if (!evidenceFingerprint) throw new Error("expected reviewable fingerprint");
	return { clusterContext: data.cluster, objectKey: entry.key, evidenceFingerprint, disposition: "expected", note: " Confirmed reader access ", reviewedAt: "2026-08-24T10:00:00Z" };
}

// Literals were computed on main's reviewModel and cockpitModel with this exact fixture. Persisted reviews
// compare against these strings, so a change here invalidates every saved rbac-v1 review.
function pinnedInspection(cluster = "kind-dev"): RbacInspectionSummary {
	// Inline literal, no cast: the type checker verifies this fixture against RbacInspectionSummary.
	const rule = { verbs: ["get"], apiGroups: [""], resources: ["pods"], resourceNames: [], nonResourceUrls: [], risks: [] };
	return {
		cluster,
		warnings: [],
		coverage: [],
		serviceAccounts: [{ cluster, name: "api", namespace: "team", age: "1d", secretsCount: 0, imagePullSecretsCount: 0, risks: [] }],
		roles: [{ cluster, kind: "Role", name: "reader", namespace: "team", age: "1d", rulesCount: 1, risks: [], rules: [rule] }],
		clusterRoles: [{ cluster, kind: "ClusterRole", name: "viewer", age: "1d", rulesCount: 1, risks: [], rules: [{ ...rule, verbs: ["list"], resources: ["namespaces"] }] }],
		roleBindings: [
			{ cluster, kind: "RoleBinding", name: "read", namespace: "team", age: "1d", roleRefKind: "Role", roleRefName: "reader", subjects: [{ kind: "ServiceAccount", name: "api", namespace: "team" }], risks: [] },
			{ cluster, kind: "RoleBinding", name: "decoy", namespace: "other", age: "1d", roleRefKind: "ClusterRole", roleRefName: "viewer", subjects: [{ kind: "ServiceAccount", name: "api", namespace: "other" }], risks: [] },
		],
		clusterRoleBindings: [{ cluster, kind: "ClusterRoleBinding", name: "view-api", age: "1d", roleRefKind: "ClusterRole", roleRefName: "viewer", subjects: [{ kind: "ServiceAccount", name: "api", namespace: "team" }], risks: [] }],
		namespaceAccess: [{ cluster, namespace: "team", serviceAccounts: 1, roles: 1, roleBindings: 1, boundSubjects: [], risks: [] }],
	};
}

describe("RBAC evidence fingerprints", () => {
	test("keep the rbac-v1 values main persisted for a Role and a ServiceAccount with inherited bindings", () => {
		const data = pinnedInspection();
		const serviceAccount = cockpitItems(data, "Service Accounts")[0];
		if (!serviceAccount) throw new Error("expected service account entry");
		expect(buildRbacEvidenceFingerprint(data, roleEntry(data))).toBe("rbac-v1-3oa15f05e1o52");
		expect(buildRbacEvidenceFingerprint(data, serviceAccount)).toBe("rbac-v1-fau57i88uz26");
	});

	test("keeps the first namespace-qualified Role and first ClusterRole in warm fingerprints", () => {
		const data = pinnedInspection();
		const [role] = data.roles;
		const [clusterRole] = data.clusterRoles;
		const [account] = cockpitItems(data, "Service Accounts");
		const [binding] = cockpitItems(data, "Bindings");
		if (!role || !clusterRole || !account || !binding) throw new Error("expected RBAC fixture entries");
		const bindingFingerprint = buildRbacEvidenceFingerprint(data, binding);

		data.roles.unshift({ ...role, namespace: "other", rules: [] });
		data.roles.push({ ...role, rules: [] });
		data.clusterRoles.push({ ...clusterRole, rules: [] });
		const index = buildRbacBindingIndex(data);

		expect(buildRbacEvidenceFingerprint(data, account, index)).toBe("rbac-v1-fau57i88uz26");
		expect(buildRbacEvidenceFingerprint(data, binding, index)).toBe(bindingFingerprint);
	});
});

describe("RBAC review model", () => {
	test("marks matching review active and changed evidence stale", () => {
		const data = inspection();
		const record = review(data);
		expect(reviewStatus(record, data, roleEntry(data))).toBe("active");
		data.roles[0]?.rules[0]?.verbs.push("list");
		expect(reviewStatus(record, data, roleEntry(data))).toBe("stale");
	});

	test("invalidates role review when its binding path changes", () => {
		const data = inspection();
		const fingerprint = buildRbacEvidenceFingerprint(data, roleEntry(data));
		const subject = firstBinding(data).subjects[0];
		if (!subject) throw new Error("expected binding subject");
		subject.name = "other";
		expect(buildRbacEvidenceFingerprint(data, roleEntry(data))).not.toBe(fingerprint);
	});

	test("invalidates binding and service-account reviews when inherited path changes", () => {
		const data = inspection();
		const binding = cockpitItems(data, "Bindings")[0];
		const account = cockpitItems(data, "Service Accounts")[0];
		if (!binding || !account) throw new Error("expected reviewable entries");
		const bindingFingerprint = buildRbacEvidenceFingerprint(data, binding);
		const accountFingerprint = buildRbacEvidenceFingerprint(data, account);
		firstBinding(data).roleRefName = "other";
		expect(buildRbacEvidenceFingerprint(data, binding)).not.toBe(bindingFingerprint);
		expect(buildRbacEvidenceFingerprint(data, account)).not.toBe(accountFingerprint);
	});

	test("excludes Namespace Access and validates trimmed notes", () => {
		const data = inspection();
		const namespace = cockpitItems(data, "Namespace Access")[0];
		if (!namespace) throw new Error("expected namespace entry");
		expect(buildRbacEvidenceFingerprint(data, namespace)).toBeUndefined();
		expect(() => upsertRbacReviewRecord([], { ...review(data), note: " \n " })).toThrow("must not be empty");
		expect(upsertRbacReviewRecord([], review(data))[0]?.note).toBe("Confirmed reader access");
	});

	test("builds deterministic review subject labels from exact RBAC relationships", () => {
		const data = inspection();
		firstBinding(data).subjects.push({ kind: "User", name: "mira" }, { kind: "Group", name: "ops" });
		const role = cockpitItems(data, "Roles")[0];
		const binding = cockpitItems(data, "Bindings")[0];
		const account = cockpitItems(data, "Service Accounts")[0];
		if (!role || !binding || !account) throw new Error("expected reviewable entries");
		expect(rbacReviewSubjectLabels(data, role)).toEqual(["Group:ops", "ServiceAccount:team/api", "User:mira"]);
		expect(rbacReviewSubjectLabels(data, binding)).toEqual(["Group:ops", "ServiceAccount:team/api", "User:mira"]);
		expect(rbacReviewSubjectLabels(data, account)).toEqual(["ServiceAccount:team/api"]);
	});

	test("upserts and removes one cluster-object pair without cascading", () => {
		const first = review(inspection("cluster-a"));
		const second = review(inspection("cluster-b"));
		const records = upsertRbacReviewRecord(upsertRbacReviewRecord([], first), second);
		expect(records).toHaveLength(2);
		const updated = upsertRbacReviewRecord(records, { ...first, disposition: "anomalous", note: "Unexpected grant" });
		expect(updated).toHaveLength(2);
		expect(updated.find((record) => record.clusterContext === "cluster-a")?.disposition).toBe("anomalous");
		expect(removeRbacReviewRecord(updated, "cluster-a", first.objectKey)).toEqual([{ ...second, note: "Confirmed reader access" }]);
	});
});
