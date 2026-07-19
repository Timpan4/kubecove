import type {
	RbacBindingSummary,
	RbacInspectionSummary,
	RbacRiskIndicator,
	RbacRoleSummary,
	RbacRuleSummary,
	RbacSubjectSummary,
	ServiceAccountSummary,
} from "@/lib/types";
import type { RbacView } from "./surfaceModel";

export type RbacRiskBucket = "all" | "high" | "medium" | "low" | "none" | "unknown";
export interface RbacCockpitState {
	riskBucket?: RbacRiskBucket;
	selectedObjectKey?: string;
}
export type { RbacVerifierHandoff } from "./handoff";
export type RbacCockpitItem =
	| { category: RbacView; key: string; name: string; namespace?: string; kind: string; risks: RbacRiskIndicator[]; findings: number; item: ServiceAccountSummary | RbacRoleSummary | RbacBindingSummary | RbacNamespaceAccessRow }
;
type RbacNamespaceAccessRow = RbacInspectionSummary["namespaceAccess"][number];

export function objectKey(category: RbacView, item: { kind?: string; name?: string; namespace?: string }): string {
	return `${category}:${item.kind ?? category}:${item.namespace ?? "_"}:${item.name ?? ("namespace" in item ? item.namespace : "_")}`;
}

export function cockpitItems(data: RbacInspectionSummary, category: RbacView): RbacCockpitItem[] {
	const rows: Array<ServiceAccountSummary | RbacRoleSummary | RbacBindingSummary | RbacNamespaceAccessRow> =
		category === "Roles" ? data.roles
			: category === "Cluster Roles" ? data.clusterRoles
			: category === "Bindings" ? [...data.roleBindings, ...data.clusterRoleBindings]
			: category === "Service Accounts" ? data.serviceAccounts
			: data.namespaceAccess;
	return rows.map((item) => {
		const risks = resolvedRisks(data, category, item);
		const name = "name" in item ? item.name : item.namespace;
		const kind = category === "Service Accounts"
			? "ServiceAccount"
			: "kind" in item
				? item.kind
				: "Namespace";
		return { category, key: objectKey(category, { kind, name, namespace: "namespace" in item ? item.namespace : undefined }), name, namespace: "namespace" in item ? item.namespace : undefined, kind, risks, findings: risks.length, item };
	}).sort((a, b) => riskWeight(b.risks) - riskWeight(a.risks) || b.findings - a.findings || `${a.namespace ?? ""}/${a.name}`.localeCompare(`${b.namespace ?? ""}/${b.name}`));
}

function resolvedRisks(
	data: RbacInspectionSummary,
	category: RbacView,
	item: ServiceAccountSummary | RbacRoleSummary | RbacBindingSummary | RbacNamespaceAccessRow,
): RbacRiskIndicator[] {
	if (category === "Bindings") {
		const binding = item as RbacBindingSummary;
		return uniqueRisks([
			...binding.risks,
			...referencedRoleRisks(data, binding),
		]);
	}
	if (category !== "Service Accounts") return item.risks;
	const account = item as ServiceAccountSummary;
	const bindings = [...data.roleBindings, ...data.clusterRoleBindings].filter(
		(binding) => bindingMatchesServiceAccount(binding, account),
	);
	const inherited = bindings.flatMap((binding) => [
		...binding.risks,
		...referencedRoleRisks(data, binding),
	]);
	const incomplete = data.coverage.filter(
		(entry) =>
			entry.status !== "complete" &&
			entry.family !== "serviceAccounts",
	);
	return uniqueRisks([
		...account.risks,
		...inherited,
		...(incomplete.length
			? [{
				level: "unknown" as const,
				label: "Incomplete RBAC coverage",
				reason: `Coverage is ${incomplete.map((entry) => `${entry.family} ${entry.status}`).join(", ")}.`,
			}]
			: []),
	]);
}

function referencedRoleRisks(
	data: RbacInspectionSummary,
	binding: RbacBindingSummary,
): RbacRiskIndicator[] {
	const roles = binding.roleRefKind === "Role" ? data.roles : data.clusterRoles;
	const role = roles.find(
		(candidate) =>
			candidate.name === binding.roleRefName &&
			(binding.roleRefKind !== "Role" || candidate.namespace === binding.namespace),
	);
	if (role) return role.risks;
	return [{
		level: "unknown",
		label: "Missing role reference",
		reason: `${binding.kind}/${binding.name} references ${binding.roleRefKind}/${binding.roleRefName}, which was not loaded.`,
	}];
}

function bindingMatchesServiceAccount(
	binding: RbacBindingSummary,
	account: ServiceAccountSummary,
): boolean {
	const automaticGroups = new Set(automaticServiceAccountGroups(account.namespace));
	return binding.subjects.some(
		(subject) =>
			(subject.kind === "ServiceAccount" &&
				subject.name === account.name &&
				subject.namespace === account.namespace) ||
			(subject.kind === "Group" && automaticGroups.has(subject.name)),
	);
}

function uniqueRisks(risks: RbacRiskIndicator[]): RbacRiskIndicator[] {
	const seen = new Set<string>();
	return risks.filter((risk) => {
		const key = `${risk.level}:${risk.label}:${risk.reason}`;
		if (seen.has(key)) return false;
		seen.add(key);
		return true;
	});
}

export function filterCockpitItems(items: RbacCockpitItem[], bucket: RbacRiskBucket, search: string): RbacCockpitItem[] {
	const needle = search.trim().toLowerCase();
	return items.filter((entry) => {
		const risk = bucket === "all" || (bucket === "none" ? entry.risks.length === 0 : entry.risks.some((item) => item.level === bucket));
		if (!risk) return false;
		return !needle || searchable(entry).includes(needle);
	});
}

export function selectedCockpitItem(items: RbacCockpitItem[], selectedKey?: string): RbacCockpitItem | undefined {
	return items.find((item) => item.key === selectedKey) ?? items[0];
}

export function ruleText(rule: RbacRuleSummary): string {
	const resources = rule.resources.map((resource) => `${rule.apiGroups.length ? rule.apiGroups.join(",") || "core" : "core"}/${resource}`).join(", ");
	return [rule.verbs.join(", "), resources, rule.nonResourceUrls.join(", ")].filter(Boolean).join(" · ");
}

export function automaticServiceAccountGroups(namespace: string): string[] {
	return ["system:serviceaccounts", `system:serviceaccounts:${namespace}`, "system:authenticated"];
}

function searchable(entry: RbacCockpitItem): string {
	const item = entry.item;
	const parts = [entry.name, entry.namespace, entry.kind];
	if ("roleRefName" in item) parts.push(item.roleRefName, item.roleRefKind, ...item.subjects.flatMap(subjectWords));
	if ("rules" in item) parts.push(...item.rules.flatMap((rule) => [...rule.verbs, ...rule.apiGroups, ...rule.resources, ...rule.nonResourceUrls]));
	return parts.filter(Boolean).join(" ").toLowerCase();
}

function subjectWords(subject: RbacSubjectSummary): string[] { return [subject.kind, subject.name, subject.namespace ?? ""]; }
function riskWeight(risks: RbacRiskIndicator[]): number { return risks.some((risk) => risk.level === "high") ? 3 : risks.some((risk) => risk.level === "medium") ? 2 : risks.length ? 1 : 0; }
