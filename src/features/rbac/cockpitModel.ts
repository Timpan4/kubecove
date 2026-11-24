import type {
	RbacAccessReviewIdentity,
	RbacBindingSummary,
	RbacInspectionSummary,
	RbacRiskIndicator,
	RbacRoleSummary,
	RbacRuleSummary,
	RbacSubjectSummary,
	ServiceAccountSummary,
} from "@/lib/types";
import { principalOf } from "./principal";
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

export interface RbacBindingIndex {
	bindings: RbacBindingSummary[];
	bindingsByNamespace: Map<string | undefined, RbacBindingSummary[]>;
	bindingsByRole: Map<string, RbacBindingSummary[]>;
	bindingsByUser: Map<string, RbacBindingSummary[]>;
	bindingsByGroup: Map<string, RbacBindingSummary[]>;
	bindingsByServiceAccount: Map<string, RbacBindingSummary[]>;
	bindingOrder: Map<RbacBindingSummary, number>;
	// Each binding's own risks followed by its referenced role's risks.
	bindingRisks: Map<RbacBindingSummary, RbacRiskIndicator[]>;
}

export function objectKey(category: RbacView, item: { kind?: string; name?: string; namespace?: string }): string {
	return `${category}:${item.kind ?? category}:${item.namespace ?? "_"}:${item.name ?? ("namespace" in item ? item.namespace : "_")}`;
}

export function cockpitItems(
	data: RbacInspectionSummary,
	category: RbacView,
	index?: RbacBindingIndex,
): RbacCockpitItem[] {
	// Bindings and Service Accounts need the binding index for risks. Bindings also take their rows from it.
	let riskIndex: RbacBindingIndex | undefined;
	let rows: Array<ServiceAccountSummary | RbacRoleSummary | RbacBindingSummary | RbacNamespaceAccessRow>;
	switch (category) {
		case "Roles":
			rows = data.roles;
			break;
		case "Cluster Roles":
			rows = data.clusterRoles;
			break;
		case "Bindings":
			riskIndex = index ?? buildRbacBindingIndex(data);
			rows = riskIndex.bindings;
			break;
		case "Service Accounts":
			riskIndex = index ?? buildRbacBindingIndex(data);
			rows = data.serviceAccounts;
			break;
		default:
			rows = data.namespaceAccess;
	}
	const keyed = rows.map((item) => {
		const risks = resolvedRisks(data, riskIndex, category, item);
		const name = "name" in item ? item.name : item.namespace;
		const kind = category === "Service Accounts"
			? "ServiceAccount"
			: "kind" in item
				? item.kind
				: "Namespace";
		const namespace = "namespace" in item ? item.namespace : undefined;
		const entry: RbacCockpitItem = { category, key: objectKey(category, { kind, name, namespace }), name, namespace, kind, risks, findings: risks.length, item };
		// Sort keys are computed once per row instead of twice per comparison.
		return { entry, weight: riskWeight(risks), label: `${namespace ?? ""}/${name}` };
	});
	keyed.sort((a, b) => b.weight - a.weight || b.entry.findings - a.entry.findings || a.label.localeCompare(b.label));
	const items: RbacCockpitItem[] = [];
	for (const { entry } of keyed) items.push(entry);
	return items;
}

function resolvedRisks(
	data: RbacInspectionSummary,
	riskIndex: RbacBindingIndex | undefined,
	category: RbacView,
	item: ServiceAccountSummary | RbacRoleSummary | RbacBindingSummary | RbacNamespaceAccessRow,
): RbacRiskIndicator[] {
	if (category === "Bindings" && riskIndex) {
		// SAFETY: cockpitItems selects only binding arrays for the Bindings category.
		const binding = item as RbacBindingSummary;
		return uniqueRisks(riskIndex.bindingRisks.get(binding) ?? binding.risks);
	}
	if (category !== "Service Accounts" || !riskIndex) return item.risks;
	// SAFETY: cockpitItems selects only serviceAccounts for the Service Accounts category.
	const account = item as ServiceAccountSummary;
	const bindings = bindingsForIdentity(riskIndex, { kind: "serviceAccount", namespace: account.namespace, name: account.name });
	const inherited = bindings.flatMap(
		(binding) => riskIndex.bindingRisks.get(binding) ?? binding.risks,
	);
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
	rolesByReference: Map<string, RbacRoleSummary>,
	binding: RbacBindingSummary,
): RbacRiskIndicator[] {
	const role = rolesByReference.get(roleReferenceKey(
		binding.roleRefKind === "Role" ? "Role" : "ClusterRole",
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

export function roleReferenceKey(kind: string, name: string, namespace: string | undefined): string {
	return kind === "Role"
		? `Role\0${optionalNamespaceKey(namespace)}\0${name}`
		: `${kind}\0${name}`;
}

function optionalNamespaceKey(namespace: string | undefined): string {
	return namespace === undefined ? "missing" : `value:${namespace}`;
}

function pushMapValue<K, V>(map: Map<K, V[]>, key: K, value: V): void {
	const values = map.get(key);
	if (values) values.push(value);
	else map.set(key, [value]);
}

export function buildRbacBindingIndex(data: RbacInspectionSummary): RbacBindingIndex {
	const rolesByReference = new Map<string, RbacRoleSummary>();
	for (const role of data.roles) {
		const key = roleReferenceKey("Role", role.name, role.namespace);
		if (!rolesByReference.has(key)) rolesByReference.set(key, role);
	}
	for (const role of data.clusterRoles) {
		const key = roleReferenceKey("ClusterRole", role.name, undefined);
		if (!rolesByReference.has(key)) rolesByReference.set(key, role);
	}

	const bindingsByNamespace = new Map<string | undefined, RbacBindingSummary[]>();
	const bindingsByRole = new Map<string, RbacBindingSummary[]>();
	const bindingsByUser = new Map<string, RbacBindingSummary[]>();
	const bindingsByGroup = new Map<string, RbacBindingSummary[]>();
	const bindingsByServiceAccount = new Map<string, RbacBindingSummary[]>();
	const bindingOrder = new Map<RbacBindingSummary, number>();
	const bindingRisks = new Map<RbacBindingSummary, RbacRiskIndicator[]>();
	const bindings = [...data.roleBindings, ...data.clusterRoleBindings];
	for (const [index, binding] of bindings.entries()) {
		bindingOrder.set(binding, index);
		pushMapValue(bindingsByNamespace, binding.namespace, binding);
		pushMapValue(bindingsByRole, roleReferenceKey(binding.roleRefKind, binding.roleRefName, binding.namespace), binding);
		bindingRisks.set(binding, [...binding.risks, ...referencedRoleRisks(rolesByReference, binding)]);
		for (const subject of binding.subjects) {
			if (subject.kind === "ServiceAccount") {
				pushMapValue(bindingsByServiceAccount, bindingServiceAccountKey(subject.namespace, subject.name), binding);
			} else if (subject.kind === "User") {
				pushMapValue(bindingsByUser, subject.name, binding);
			} else if (subject.kind === "Group") {
				pushMapValue(bindingsByGroup, subject.name, binding);
			}
		}
	}

	return { bindings, bindingsByNamespace, bindingsByRole, bindingsByUser, bindingsByGroup, bindingsByServiceAccount, bindingOrder, bindingRisks };
}

export function bindingsForIdentity(
	index: RbacBindingIndex,
	identity: RbacAccessReviewIdentity,
): RbacBindingSummary[] {
	const bindings = new Set<RbacBindingSummary>();
	const { username, account, groups } = principalOf(identity);

	if (username !== undefined) {
		for (const binding of index.bindingsByUser.get(username) ?? []) bindings.add(binding);
	}
	if (account) {
		for (const binding of index.bindingsByServiceAccount.get(bindingServiceAccountKey(account.namespace, account.name)) ?? []) {
			bindings.add(binding);
		}
	}
	for (const group of groups) {
		for (const binding of index.bindingsByGroup.get(group) ?? []) bindings.add(binding);
	}
	return Array.from(bindings).sort(
		(a, b) => (index.bindingOrder.get(a) ?? 0) - (index.bindingOrder.get(b) ?? 0),
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

function searchable(entry: RbacCockpitItem): string {
	const item = entry.item;
	const parts = [entry.name, entry.namespace, entry.kind];
	if ("roleRefName" in item) parts.push(item.roleRefName, item.roleRefKind, ...item.subjects.flatMap(subjectWords));
	if ("rules" in item) parts.push(...item.rules.flatMap((rule) => [...rule.verbs, ...rule.apiGroups, ...rule.resources, ...rule.nonResourceUrls]));
	return parts.filter(Boolean).join(" ").toLowerCase();
}

function subjectWords(subject: RbacSubjectSummary): string[] { return [subject.kind, subject.name, subject.namespace ?? ""]; }
function riskWeight(risks: RbacRiskIndicator[]): number { return risks.some((risk) => risk.level === "high") ? 3 : risks.some((risk) => risk.level === "medium") ? 2 : risks.length ? 1 : 0; }
