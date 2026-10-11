import assert from "node:assert/strict";
import { bench, describe } from "vitest";
import type { RbacRiskBucket } from "@/features/rbac/cockpitModel";
import { cockpitItems, filterCockpitItems } from "@/features/rbac/cockpitModel";
import { observedPermissions } from "@/features/rbac/observedPermissions";
import {
	bindingSources,
	queueSummary,
	queueTags,
} from "@/features/rbac/queueModel";
import type { RbacReviewRecord } from "@/features/rbac/reviewModel";
import {
	buildRbacEvidenceFingerprint,
	findRbacReviewRecord,
	reviewStatus,
} from "@/features/rbac/reviewModel";
import type { RbacView } from "@/features/rbac/surfaceModel";
import type { RbacInspectionSummary } from "@/lib/types";
import { rbacInspection, rbacRoleBindings } from "./rbac-benchmark-fixtures";

const activeCategories: RbacView[] = [
	"Namespace Access",
	"Roles",
	"Cluster Roles",
	"Bindings",
	"Service Accounts",
];
const retainedItems = activeCategories.map((category) => ({
	category,
	items: cockpitItems(rbacInspection, category),
}));
const allServiceAccounts = cockpitItems(rbacInspection, "Service Accounts");
const riskBuckets: RbacRiskBucket[] = [
	"all",
	"high",
	"medium",
	"low",
	"none",
	"unknown",
];
const partialInspection: RbacInspectionSummary = {
	...rbacInspection,
	warnings: ["Role access is partial"],
	coverage: rbacInspection.coverage.map((coverage) =>
		coverage.family === "roles" ? { ...coverage, status: "partial" } : coverage,
	),
	roleBindings: rbacInspection.roleBindings.map((binding, index) =>
		index === 0 ? { ...binding, roleRefName: "missing-role" } : binding,
	),
};
const visibleServiceAccounts = cockpitItems(
	rbacInspection,
	"Service Accounts",
).slice(0, 100);
const reviewRecords: RbacReviewRecord[] = visibleServiceAccounts
	.filter((_, index) => index % 5 === 0)
	.map((entry) => ({
		clusterContext: rbacInspection.cluster,
		objectKey: entry.key,
		evidenceFingerprint:
			buildRbacEvidenceFingerprint(rbacInspection, entry) ?? "",
		disposition: "expected",
		note: "Reviewed in deterministic fixture",
		reviewedAt: "2026-10-11T12:00:00Z",
	}));
const reviewRows = visibleServiceAccounts.slice(0, 50);
const firstServiceAccount = visibleServiceAccounts[0];
const secondPageServiceAccounts = visibleServiceAccounts.slice(50, 100);
const firstReviewRecord = reviewRecords[0];
assert(firstReviewRecord && firstServiceAccount);
const reorderedInspection: RbacInspectionSummary = {
	...rbacInspection,
	roles: rbacInspection.roles
		.map((role) => ({
			...role,
			rules: role.rules
				.map((rule) => ({
					...rule,
					verbs: [...rule.verbs].reverse(),
					resources: [...rule.resources].reverse(),
				}))
				.reverse(),
		}))
		.reverse(),
	roleBindings: [...rbacInspection.roleBindings].reverse(),
	clusterRoleBindings: [...rbacInspection.clusterRoleBindings].reverse(),
};
assert.equal(
	reviewStatus(firstReviewRecord, rbacInspection, firstServiceAccount),
	"active",
);
assert.equal(
	buildRbacEvidenceFingerprint(reorderedInspection, firstServiceAccount),
	firstReviewRecord.evidenceFingerprint,
);
assert.equal(
	reviewStatus(
		{ ...firstReviewRecord, evidenceFingerprint: "old-policy" },
		rbacInspection,
		firstServiceAccount,
	),
	"stale",
);
assert.equal(
	findRbacReviewRecord(
		[{ ...firstReviewRecord, clusterContext: "other" }],
		rbacInspection.cluster,
		firstServiceAccount.key,
	),
	undefined,
);
assert.equal(
	reviewStatus(
		{ ...firstReviewRecord, objectKey: "other" },
		rbacInspection,
		firstServiceAccount,
	),
	"stale",
);
// Retain current, stale, and wrong-context records in the mounted-row workload.
reviewRecords[1] = { ...reviewRecords[1], evidenceFingerprint: "old-policy" };
reviewRecords[2] = { ...reviewRecords[2], clusterContext: "other" };
const identities = [
	{ kind: "serviceAccount", name: "account-42", namespace: "namespace-42" },
	{ kind: "group", name: "system:authenticated" },
	{ kind: "user", name: "operator", groups: ["system:authenticated"] },
] as const;
const inspectionBefore = JSON.stringify(rbacInspection);
for (const identity of identities) {
	const result = observedPermissions(rbacInspection, {
		...identity,
		groups: "groups" in identity ? [...identity.groups] : undefined,
	});
	assert.equal(result.unknown, false);
	assert(result.grants.some((grant) => grant.scope === "cluster"));
	assert(result.grants.some((grant) => grant.scope === "namespace"));
	assert.deepEqual(result.grants[0].verbs, ["*"]);
	for (const grant of result.grants) {
		assert.deepEqual(
			grant.roles,
			[...new Set(grant.roles)].sort((left, right) =>
				left.localeCompare(right),
			),
		);
		assert.deepEqual(
			grant.bindings,
			[...new Set(grant.bindings)].sort((left, right) =>
				left.localeCompare(right),
			),
		);
		assert.equal(
			grant.scope === "namespace" && grant.nonResourceUrls.length > 0,
			false,
		);
	}
}
const partialPermissions = observedPermissions(
	partialInspection,
	identities[1],
);
assert.equal(partialPermissions.unknown, true);
assert(
	partialPermissions.reason?.includes("referenced role") &&
		partialPermissions.reason.includes("partial"),
);
assert(
	cockpitItems(partialInspection, "Service Accounts").some((entry) =>
		entry.risks.some((risk) => risk.level === "unknown"),
	),
);
assert.equal(filterCockpitItems(allServiceAccounts, "all", "").length, 500);
assert.equal(filterCockpitItems(allServiceAccounts, "unknown", "").length, 0);
assert.equal(JSON.stringify(rbacInspection), inspectionBefore);
const duplicateRole = rbacInspection.roles[0];
assert(duplicateRole);
const duplicateRoleInspection = {
	...rbacInspection,
	roles: [
		...rbacInspection.roles,
		{
			...duplicateRole,
			rules: [{ ...duplicateRole.rules[0], verbs: ["watch"] }],
		},
	],
};
assert.deepEqual(
	observedPermissions(duplicateRoleInspection, identities[1]).grants[0].verbs,
	["watch"],
);

assert.equal(activeCategories.length, 5);
assert.equal(rbacRoleBindings.length, 1_500);
assert.equal(rbacInspection.coverage.length, 5);
assert.equal(reviewRows.length, 50);
assert.equal(reviewRecords.length, 20);
assert(firstServiceAccount);

describe("RBAC surface F39–F42", () => {
	bench("F39 cold cockpit shaping across active categories", () => {
		for (const category of activeCategories) {
			const items = cockpitItems(rbacInspection, category);
			filterCockpitItems(items, "high", "");
		}
	});

	bench("F39 retained category search and risk filtering", () => {
		for (const { items } of retainedItems) {
			filterCockpitItems(items, "all", "account");
			filterCockpitItems(items, "high", "namespace");
		}
	});
	bench("F39 partial coverage and missing-reference cold cockpit", () => {
		for (const category of activeCategories)
			cockpitItems(partialInspection, category);
	});

	bench(
		"F40 observed permission joins for service account, group, and user",
		() => {
			observedPermissions(rbacInspection, {
				kind: "serviceAccount",
				name: "account-42",
				namespace: "namespace-42",
			});
			observedPermissions(rbacInspection, {
				kind: "group",
				name: "system:authenticated",
			});
			observedPermissions(rbacInspection, {
				kind: "user",
				name: "operator",
				groups: ["system:authenticated"],
			});
		},
	);

	bench(
		"F41 reviewed-row lookup, evidence fingerprints, and absent-review fast path",
		() => {
			for (const entry of reviewRows) {
				const record = findRbacReviewRecord(
					reviewRecords,
					rbacInspection.cluster,
					entry.key,
				);
				reviewStatus(record, rbacInspection, entry);
			}
		},
	);

	bench(
		"F42 visible queue rows, tags, summaries, and selected binding sources",
		() => {
			for (const bucket of riskBuckets)
				filterCockpitItems(allServiceAccounts, bucket, "");
			for (const entry of reviewRows) {
				queueTags(rbacInspection, entry);
				queueSummary(entry);
			}
			if (firstServiceAccount)
				bindingSources(rbacInspection, firstServiceAccount);
			for (const entry of secondPageServiceAccounts) {
				queueTags(rbacInspection, entry);
				queueSummary(entry);
			}
		},
	);
});
