import type {
	RbacBindingSummary,
	RbacNamespaceAccessSummary,
	RbacRoleSummary,
	ServiceAccountSummary,
} from "@/lib/types";
import { bindingsForIdentity, roleReferenceKey, type RbacBindingIndex, type RbacCockpitItem } from "./cockpitModel";
import { subjectListLabel } from "./risk";

export function queueTags(
	index: RbacBindingIndex,
	entry: RbacCockpitItem,
): string[] {
	if (
		entry.category === "Service Accounts" &&
		isServiceAccountSummary(entry.item)
	) {
		const account = entry.item;
		return [
			`${bindingSources(index, entry).length} bindings`,
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
	index: RbacBindingIndex,
	entry: RbacCockpitItem,
): RbacBindingSummary[] {
	if (
		entry.category === "Service Accounts" &&
		isServiceAccountSummary(entry.item)
	) {
		const { name, namespace } = entry.item;
		return bindingsForIdentity(index, { kind: "serviceAccount", name, namespace });
	}
	if (
		(entry.category === "Roles" || entry.category === "Cluster Roles") &&
		isRbacRoleSummary(entry.item)
	) {
		const role = entry.item;
		return index.bindingsByRole.get(roleReferenceKey(role.kind, role.name, role.namespace)) ?? [];
	}
	if (entry.category === "Bindings" && isRbacBindingSummary(entry.item))
		return [entry.item];
	return index.bindingsByNamespace.get(entry.namespace) ?? [];
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
