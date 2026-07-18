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
		const risks = item.risks;
		const name = "name" in item ? item.name : item.namespace;
		return { category, key: objectKey(category, { kind: "kind" in item ? item.kind : "Namespace", name, namespace: "namespace" in item ? item.namespace : undefined }), name, namespace: "namespace" in item ? item.namespace : undefined, kind: "kind" in item ? item.kind : "Namespace", risks, findings: risks.length, item };
	}).sort((a, b) => riskWeight(b.risks) - riskWeight(a.risks) || b.findings - a.findings || `${a.namespace ?? ""}/${a.name}`.localeCompare(`${b.namespace ?? ""}/${b.name}`));
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
