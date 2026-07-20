import type { RbacAccessReviewIdentity, RbacBindingSummary, RbacInspectionSummary, RbacRoleSummary, RbacRuleSummary, RbacSubjectSummary } from "@/lib/types";

export interface ObservedGrant {
	verbs: string[];
	apiGroups: string[];
	resources: string[];
	resourceNames: string[];
	nonResourceUrls: string[];
	scope: "cluster" | "namespace";
	namespace?: string;
	roles: string[];
	bindings: string[];
}

export interface ObservedPermissionResult {
	grants: ObservedGrant[];
	unknown: boolean;
	reason?: string;
}

export function observedPermissions(
	inspection: RbacInspectionSummary,
	identity: { kind: "serviceAccount" | "user" | "group"; name: string; namespace?: string; groups?: string[] },
): ObservedPermissionResult {
	const coverage = new Map(inspection.coverage.map((item) => [item.family, item.status]));
	const incomplete = (["roles", "clusterRoles", "roleBindings", "clusterRoleBindings"] as const)
		.filter((family) => coverage.get(family) !== "complete");
	const roles = new Map<string, RbacRoleSummary>();
	for (const role of [...inspection.roles, ...inspection.clusterRoles]) roles.set(roleKey(role.kind, role.name, role.namespace), role);
	const grants: ObservedGrant[] = [];
	let missing = false;
	for (const binding of [...inspection.roleBindings, ...inspection.clusterRoleBindings]) {
		if (!binding.subjects.some((subject) => matchesSubject(subject, identity))) continue;
		const role = roles.get(roleKey(binding.roleRefKind, binding.roleRefName, binding.roleRefKind === "Role" ? binding.namespace : undefined));
		if (!role) { missing = true; continue; }
		const scope = binding.kind === "ClusterRoleBinding" ? "cluster" : "namespace";
		for (const rule of role.rules) {
			const observed = grant(rule, scope, binding.namespace, role, binding);
			if (observed) grants.push(observed);
		}
	}
	const reasons = [
		incomplete.length ? `Coverage is partial or unavailable for ${incomplete.join(", ")}.` : "",
		missing ? "A referenced role was not loaded." : "",
	].filter(Boolean);
	return { grants: merge(grants), unknown: reasons.length > 0, reason: reasons.join(" ") || undefined };
}

export function identityDefaults(kind: "serviceAccount" | "user" | "group", name = ""): string[] {
	return kind === "user" ? [name === "system:anonymous" ? "system:unauthenticated" : "system:authenticated"] : [];
}

export function inspectorIdentity(identity: RbacAccessReviewIdentity | undefined): {
	kind: "serviceAccount" | "user" | "group";
	name: string;
	namespace?: string;
	groups?: string[];
} | null {
	if (!identity) return null;
	if (identity.kind === "serviceAccount") return { kind: "serviceAccount", name: identity.name, namespace: identity.namespace };
	if (identity.kind === "group") return { kind: "group", name: identity.group };
	return { kind: "user", name: identity.username, groups: identity.groups };
}

function matchesSubject(subject: RbacSubjectSummary, identity: { kind: string; name: string; namespace?: string; groups?: string[] }): boolean {
	if (identity.kind === "serviceAccount") {
		if (subject.kind === "ServiceAccount") {
			return subject.name === identity.name && subject.namespace === identity.namespace;
		}
		const groups = new Set([
			"system:serviceaccounts",
			`system:serviceaccounts:${identity.namespace ?? ""}`,
			"system:authenticated",
		]);
		return subject.kind === "Group" && groups.has(subject.name);
	}
	if (identity.kind === "user") {
		return (subject.kind === "User" && subject.name === identity.name) ||
			(subject.kind === "Group" && new Set(identity.groups ?? []).has(subject.name));
	}
	return subject.kind === "Group" && subject.name === identity.name;
}
function grant(rule: RbacRuleSummary, scope: "cluster" | "namespace", namespace: string | undefined, role: RbacRoleSummary, binding: RbacBindingSummary): ObservedGrant | null {
	const nonResourceUrls = scope === "namespace" ? [] : rule.nonResourceUrls;
	if (rule.resources.length === 0 && nonResourceUrls.length === 0) return null;
	return { verbs: rule.verbs, apiGroups: rule.apiGroups, resources: rule.resources, resourceNames: rule.resourceNames, nonResourceUrls, scope, namespace: scope === "namespace" ? namespace : undefined, roles: [roleKey(role.kind, role.name, role.namespace)], bindings: [bindingKey(binding)] };
}
function merge(grants: ObservedGrant[]): ObservedGrant[] {
	const merged = new Map<string, ObservedGrant>();
	for (const item of grants) {
		const key = JSON.stringify([item.verbs, item.apiGroups, item.resources, item.resourceNames, item.nonResourceUrls, item.scope, item.namespace]);
		const current = merged.get(key);
		if (current) { current.roles = unique([...current.roles, ...item.roles]); current.bindings = unique([...current.bindings, ...item.bindings]); } else merged.set(key, { ...item, roles: [...item.roles], bindings: [...item.bindings] });
	}
	return [...merged.values()];
}
function roleKey(kind: string, name: string, namespace?: string): string { return `${kind}/${namespace ?? "_"}/${name}`; }
function bindingKey(binding: RbacBindingSummary): string { return `${binding.kind}/${binding.namespace ?? "_"}/${binding.name}`; }
function unique(items: string[]): string[] { return [...new Set(items)].sort((a, b) => a.localeCompare(b)); }
