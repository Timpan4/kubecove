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

interface RbacRiskIndex {
	bindingsByGroup: Map<string, RbacBindingSummary[]>;
	bindingsByServiceAccount: Map<string, RbacBindingSummary[]>;
	bindingOrder: Map<RbacBindingSummary, number>;
	rolesByReference: Map<string, RbacRoleSummary>;
}

export function objectKey(category: RbacView, item: { kind?: string; name?: string; namespace?: string }): string {
	return `${category}:${item.kind ?? category}:${item.namespace ?? "_"}:${item.name ?? ("namespace" in item ? item.namespace : "_")}`;
}

export function cockpitItems(data: RbacInspectionSummary, category: RbacView): RbacCockpitItem[] {
	const riskIndex = category === "Bindings" || category === "Service Accounts"
		? buildRbacRiskIndex(data)
		: undefined;
	const rows: Array<ServiceAccountSummary | RbacRoleSummary | RbacBindingSummary | RbacNamespaceAccessRow> =
		category === "Roles" ? data.roles
			: category === "Cluster Roles" ? data.clusterRoles
			: category === "Bindings" ? [...data.roleBindings, ...data.clusterRoleBindings]
			: category === "Service Accounts" ? data.serviceAccounts
			: data.namespaceAccess;
	return rows.map((item) => {
		const risks = resolvedRisks(data, riskIndex, category, item);
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
	riskIndex: RbacRiskIndex | undefined,
	category: RbacView,
	item: ServiceAccountSummary | RbacRoleSummary | RbacBindingSummary | RbacNamespaceAccessRow,
): RbacRiskIndicator[] {
	if (category === "Bindings" && riskIndex) {
		// SAFETY: cockpitItems selects only binding arrays for the Bindings category.
		const binding = item as RbacBindingSummary;
		return uniqueRisks([
			...binding.risks,
			...referencedRoleRisks(riskIndex, binding),
		]);
	}
	if (category !== "Service Accounts" || !riskIndex) return item.risks;
	// SAFETY: cockpitItems selects only serviceAccounts for the Service Accounts category.
	const account = item as ServiceAccountSummary;
	const bindings = bindingsForServiceAccount(riskIndex, account);
	const inherited = bindings.flatMap((binding) => [
		...binding.risks,
		...referencedRoleRisks(riskIndex, binding),
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
	riskIndex: RbacRiskIndex,
	binding: RbacBindingSummary,
): RbacRiskIndicator[] {
	const role = riskIndex.rolesByReference.get(roleReferenceKey(
		binding.roleRefKind,
		binding.roleRefName,
		binding.namespace,
	));
	if (role) return role.risks;
	return [{
		level: "unknown",
		label: "Missing role reference",
		reason: `${binding.kind}/${binding.name} references ${binding.roleRefKind}/${binding.roleRefName}, which was not loaded.`,
	}];
}

function bindingServiceAccountKey(namespace: string | undefined, name: string): string {
	return `${optionalNamespaceKey(namespace)}\0${name}`;
}

function roleReferenceKey(kind: string, name: string, namespace: string | undefined): string {
	return kind === "Role"
		? `Role\0${optionalNamespaceKey(namespace)}\0${name}`
		: `ClusterRole\0${name}`;
}

function optionalNamespaceKey(namespace: string | undefined): string {
	return namespace === undefined ? "missing" : `value:${namespace}`;
}

function pushMapValue<K, V>(map: Map<K, V[]>, key: K, value: V): void {
	const values = map.get(key);
	if (values) values.push(value);
	else map.set(key, [value]);
}

function buildRbacRiskIndex(data: RbacInspectionSummary): RbacRiskIndex {
	const rolesByReference = new Map<string, RbacRoleSummary>();
	for (const role of data.roles) {
		const key = roleReferenceKey("Role", role.name, role.namespace);
		if (!rolesByReference.has(key)) rolesByReference.set(key, role);
	}
	for (const role of data.clusterRoles) {
		const key = roleReferenceKey("ClusterRole", role.name, undefined);
		if (!rolesByReference.has(key)) rolesByReference.set(key, role);
	}

	const bindingsByGroup = new Map<string, RbacBindingSummary[]>();
	const bindingsByServiceAccount = new Map<string, RbacBindingSummary[]>();
	const bindingOrder = new Map<RbacBindingSummary, number>();
	const bindings = [...data.roleBindings, ...data.clusterRoleBindings];
	for (const [index, binding] of bindings.entries()) {
		bindingOrder.set(binding, index);
		for (const subject of binding.subjects) {
			if (subject.kind === "ServiceAccount") {
				pushMapValue(
					bindingsByServiceAccount,
					bindingServiceAccountKey(subject.namespace, subject.name),
					binding,
				);
			} else if (subject.kind === "Group") {
				pushMapValue(bindingsByGroup, subject.name, binding);
			}
		}
	}

	return { bindingsByGroup, bindingsByServiceAccount, bindingOrder, rolesByReference };
}

function bindingsForServiceAccount(
	riskIndex: RbacRiskIndex,
	account: ServiceAccountSummary,
): RbacBindingSummary[] {
	const bindings = new Set(
		riskIndex.bindingsByServiceAccount.get(
			bindingServiceAccountKey(account.namespace, account.name),
		) ?? [],
	);
	for (const group of automaticServiceAccountGroups(account.namespace)) {
		for (const binding of riskIndex.bindingsByGroup.get(group) ?? []) bindings.add(binding);
	}
	return Array.from(bindings).sort(
		(a, b) => (riskIndex.bindingOrder.get(a) ?? 0) - (riskIndex.bindingOrder.get(b) ?? 0),
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
