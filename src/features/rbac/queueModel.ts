import type {
	RbacBindingSummary,
	RbacInspectionSummary,
	RbacNamespaceAccessSummary,
	RbacRoleSummary,
	ServiceAccountSummary,
} from "@/lib/types";
import type { RbacCockpitItem } from "./cockpitModel";
import { serviceAccountMatcher } from "./principal";
import { subjectListLabel } from "./risk";

export function queueTags(
	inspection: RbacInspectionSummary,
	entry: RbacCockpitItem,
): string[] {
	if (
		entry.category === "Service Accounts" &&
		isServiceAccountSummary(entry.item)
	) {
		const account = entry.item;
		return [
			`${bindingSources(inspection, entry).length} bindings`,
			`${account.secretsCount} secrets`,
			account.automountToken === false ? "token off" : "token posture",
		];
	}
	if (
		(entry.category === "Roles" || entry.category === "Cluster Roles") &&
		isRbacRoleSummary(entry.item)
	) {
		const role = entry.item;
		return [`${role.rulesCount} rules`, role.namespace ?? "cluster-wide"];
	}
	if (entry.category === "Bindings" && isRbacBindingSummary(entry.item)) {
		const binding = entry.item;
		return [
			`${binding.subjects.length} subjects`,
			`${binding.roleRefKind}/${binding.roleRefName}`,
		];
	}
	if (!isRbacNamespaceAccessSummary(entry.item)) return [];
	const namespaceAccess = entry.item;
	return [
		`${namespaceAccess.serviceAccounts} identities`,
		`${namespaceAccess.roleBindings} bindings`,
	];
}

export function queueSummary(entry: RbacCockpitItem): string {
	if (entry.risks.length) {
		return entry.risks
			.slice(0, 2)
			.map((risk) => risk.label)
			.join(" · ");
	}
	if ("subjects" in entry.item) return subjectListLabel(entry.item.subjects, 2);
	return "No policy heuristic flags for this object.";
}

export function bindingSources(
	inspection: RbacInspectionSummary,
	entry: RbacCockpitItem,
): RbacBindingSummary[] {
	const bindings = [
		...inspection.roleBindings,
		...inspection.clusterRoleBindings,
	];
	if (
		entry.category === "Service Accounts" &&
		isServiceAccountSummary(entry.item)
	) {
		const applies = serviceAccountMatcher(entry.item.namespace, entry.item.name);
		return bindings.filter((binding) => binding.subjects.some(applies));
	}
	if (
		(entry.category === "Roles" || entry.category === "Cluster Roles") &&
		isRbacRoleSummary(entry.item)
	) {
		const role = entry.item;
		return bindings.filter(
			(binding) =>
				binding.roleRefKind === role.kind &&
				binding.roleRefName === role.name &&
				(role.kind !== "Role" || binding.namespace === role.namespace),
		);
	}
	if (entry.category === "Bindings" && isRbacBindingSummary(entry.item))
		return [entry.item];
	return bindings.filter((binding) => binding.namespace === entry.namespace);
}

function isRbacRoleSummary(
	item: RbacCockpitItem["item"],
): item is RbacRoleSummary {
	return "rules" in item;
}

function isRbacBindingSummary(
	item: RbacCockpitItem["item"],
): item is RbacBindingSummary {
	return "roleRefKind" in item;
}

function isServiceAccountSummary(
	item: RbacCockpitItem["item"],
): item is ServiceAccountSummary {
	return "imagePullSecretsCount" in item;
}

function isRbacNamespaceAccessSummary(
	item: RbacCockpitItem["item"],
): item is RbacNamespaceAccessSummary {
	return "roleBindings" in item;
}
