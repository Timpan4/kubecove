import type {
	RbacBindingSummary,
	RbacInspectionSummary,
	RbacRiskIndicator,
	RbacRoleSummary,
	RbacRuleSummary,
	RbacSubjectSummary,
	ServiceAccountSummary,
} from "@/lib/types";
import type { WorkspaceRbacReview } from "@/lib/workspace-model";
import type { RbacCockpitItem } from "./cockpitModel";
import type { RbacView } from "./surfaceModel";

export const REVIEWABLE_RBAC_CATEGORIES = [
	"Roles",
	"Cluster Roles",
	"Bindings",
	"Service Accounts",
] as const;

export type ReviewableRbacCategory = (typeof REVIEWABLE_RBAC_CATEGORIES)[number];
export type RbacReviewDisposition = WorkspaceRbacReview["disposition"];
export type RbacReviewStatus = "active" | "stale" | "none";
export type RbacReviewRecord = WorkspaceRbacReview;
export type RbacReviewRecordInput = RbacReviewRecord;

export function isReviewableRbacCategory(category: RbacView): category is ReviewableRbacCategory {
	return category !== "Namespace Access";
}

export function buildRbacEvidenceFingerprint(
	inspection: RbacInspectionSummary,
	entry: RbacCockpitItem,
): string | undefined {
	if (!isReviewableRbacCategory(entry.category)) return undefined;
	return `rbac-v1-${fnv1a(JSON.stringify(evidenceFor(inspection, entry)))}`;
}

export function reviewStatus(
	record: RbacReviewRecord | undefined,
	inspection: RbacInspectionSummary,
	entry: RbacCockpitItem,
): RbacReviewStatus {
	if (!record) return "none";
	const fingerprint = buildRbacEvidenceFingerprint(inspection, entry);
	return record.clusterContext === inspection.cluster &&
		record.objectKey === entry.key &&
		fingerprint === record.evidenceFingerprint
		? "active"
		: "stale";
}

export function rbacReviewSubjectLabels(
	inspection: RbacInspectionSummary,
	entry: RbacCockpitItem,
): string[] {
	if (!isReviewableRbacCategory(entry.category)) return [];
	if (entry.category === "Service Accounts") {
		const account = entry.item as ServiceAccountSummary;
		return [subjectLabel({ kind: "ServiceAccount", name: account.name, namespace: account.namespace })];
	}
	const bindings = entry.category === "Bindings"
		? [entry.item as RbacBindingSummary]
		: relevantBindings(inspection, entry.item as RbacRoleSummary);
	return Array.from(new Set(bindings.flatMap((binding) => binding.subjects.map(subjectLabel)))).sort();
}

export function upsertRbacReviewRecord(
	records: RbacReviewRecord[],
	input: RbacReviewRecordInput,
): RbacReviewRecord[] {
	const record = normalizedReviewRecord(input);
	const index = records.findIndex((candidate) =>
		candidate.clusterContext === record.clusterContext && candidate.objectKey === record.objectKey,
	);
	if (index < 0) return [...records, record];
	return records.map((candidate, candidateIndex) => candidateIndex === index ? record : candidate);
}

export function removeRbacReviewRecord(
	records: RbacReviewRecord[],
	clusterContext: string,
	objectKey: string,
): RbacReviewRecord[] {
	return records.filter((record) => record.clusterContext !== clusterContext || record.objectKey !== objectKey);
}

function normalizedReviewRecord(input: RbacReviewRecordInput): RbacReviewRecord {
	const note = input.note.trim();
	if (!note) throw new Error("RBAC review note must not be empty.");
	return { ...input, note };
}

function evidenceFor(inspection: RbacInspectionSummary, entry: RbacCockpitItem): unknown {
	const common = {
		cluster: inspection.cluster,
		objectKey: entry.key,
		category: entry.category,
		identity: { kind: entry.kind, namespace: entry.namespace ?? null, name: entry.name },
		scope: entry.namespace ?? "cluster",
		risks: normalizedRisks(entry.risks),
	};
	if (entry.category === "Roles" || entry.category === "Cluster Roles") {
		const role = entry.item as RbacRoleSummary;
		return {
			...common,
			role: normalizedRole(role),
			bindingPaths: relevantBindings(inspection, role)
				.map((binding) => normalizedBinding(binding, role))
				.sort(compareJson),
		};
	}
	if (entry.category === "Bindings") {
		const binding = entry.item as RbacBindingSummary;
		return { ...common, binding: normalizedBinding(binding, roleForBinding(inspection, binding)) };
	}
	if (entry.category === "Service Accounts") {
		const account = entry.item as ServiceAccountSummary;
		return {
			...common,
			serviceAccount: {
				automountToken: account.automountToken ?? null,
				secretsCount: account.secretsCount,
				imagePullSecretsCount: account.imagePullSecretsCount,
				inheritedPaths: inheritedPaths(inspection, account),
			},
		};
	}
	return common;
}

function inheritedPaths(inspection: RbacInspectionSummary, account: ServiceAccountSummary): unknown[] {
	return relevantBindings(inspection, account)
		.map((binding) => normalizedBinding(binding, roleForBinding(inspection, binding)))
		.sort(compareJson);
}

function relevantBindings(
	inspection: RbacInspectionSummary,
	item: ServiceAccountSummary | RbacRoleSummary,
): RbacBindingSummary[] {
	const bindings = [...inspection.roleBindings, ...inspection.clusterRoleBindings];
	if ("rules" in item) {
		return bindings.filter((binding) =>
			binding.roleRefKind === item.kind &&
			binding.roleRefName === item.name &&
			(binding.roleRefKind !== "Role" || binding.namespace === item.namespace),
		);
	}
	const groups = new Set([
		"system:serviceaccounts",
		`system:serviceaccounts:${item.namespace}`,
		"system:authenticated",
	]);
	return bindings.filter((binding) => binding.subjects.some((subject) =>
		(subject.kind === "ServiceAccount" && subject.name === item.name && subject.namespace === item.namespace) ||
		(subject.kind === "Group" && groups.has(subject.name)),
	));
}

function roleForBinding(inspection: RbacInspectionSummary, binding: RbacBindingSummary): RbacRoleSummary | undefined {
	const roles = binding.roleRefKind === "Role" ? inspection.roles : inspection.clusterRoles;
	return roles.find((role) =>
		role.name === binding.roleRefName &&
		(binding.roleRefKind !== "Role" || role.namespace === binding.namespace),
	);
}

function normalizedBinding(binding: RbacBindingSummary, role: RbacRoleSummary | undefined): unknown {
	return {
		identity: { kind: binding.kind, namespace: binding.namespace ?? null, name: binding.name },
		roleRef: { kind: binding.roleRefKind, name: binding.roleRefName },
		subjects: binding.subjects.map(normalizedSubject).sort(compareJson),
		risks: normalizedRisks(binding.risks),
		inheritedRole: role ? normalizedRole(role) : { missing: true },
	};
}

function normalizedRole(role: RbacRoleSummary): unknown {
	return {
		identity: { kind: role.kind, namespace: role.namespace ?? null, name: role.name },
		rules: role.rules.map(normalizedRule).sort(compareJson),
		risks: normalizedRisks(role.risks),
	};
}

function normalizedRule(rule: RbacRuleSummary): unknown {
	return {
		verbs: [...rule.verbs].sort(),
		apiGroups: [...rule.apiGroups].sort(),
		resources: [...rule.resources].sort(),
		resourceNames: [...rule.resourceNames].sort(),
		nonResourceUrls: [...rule.nonResourceUrls].sort(),
		risks: normalizedRisks(rule.risks),
	};
}

function normalizedSubject(subject: RbacSubjectSummary): unknown {
	return { kind: subject.kind, namespace: subject.namespace ?? null, name: subject.name };
}

function subjectLabel(subject: RbacSubjectSummary): string {
	return subject.namespace ? `${subject.kind}:${subject.namespace}/${subject.name}` : `${subject.kind}:${subject.name}`;
}

function normalizedRisks(risks: RbacRiskIndicator[]): unknown[] {
	return risks.map((risk) => ({ level: risk.level, label: risk.label, reason: risk.reason })).sort(compareJson);
}

function compareJson(left: unknown, right: unknown): number {
	return JSON.stringify(left).localeCompare(JSON.stringify(right));
}

function fnv1a(value: string): string {
	let hash = 0xcbf29ce484222325n;
	for (let index = 0; index < value.length; index += 1) {
		hash ^= BigInt(value.charCodeAt(index));
		hash = BigInt.asUintN(64, hash * 0x100000001b3n);
	}
	return hash.toString(36);
}
