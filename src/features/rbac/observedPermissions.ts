import type { RbacAccessReviewIdentity, RbacBindingSummary, RbacInspectionSummary, RbacRoleSummary, RbacRuleSummary } from "@/lib/types";
import { subjectMatcher } from "./principal";

export interface InspectorIdentity {
	kind: "serviceAccount" | "user" | "group";
	name: string;
	namespace?: string;
	groups?: string[];
}

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

/** Grants for the same identity the verifier would review. */
export function observedPermissions(
	inspection: RbacInspectionSummary,
	identity: InspectorIdentity,
): ObservedPermissionResult {
	const applies = subjectMatcher(reviewIdentity(identity));
	const coverage = new Map(inspection.coverage.map((item) => [item.family, item.status]));
	const incomplete = (["roles", "clusterRoles", "roleBindings", "clusterRoleBindings"] as const)
		.filter((family) => coverage.get(family) !== "complete");
	const roles = new Map<string, RbacRoleSummary>();
	for (const role of [...inspection.roles, ...inspection.clusterRoles]) roles.set(roleKey(role.kind, role.name, role.namespace), role);
	const grants: ObservedGrant[] = [];
	let missing = false;
	for (const binding of [...inspection.roleBindings, ...inspection.clusterRoleBindings]) {
		if (!binding.subjects.some(applies)) continue;
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

export function inspectorIdentity(identity: RbacAccessReviewIdentity | undefined): InspectorIdentity | null {
	if (!identity) return null;
	if (identity.kind === "serviceAccount") return { kind: "serviceAccount", name: identity.name, namespace: identity.namespace };
	if (identity.kind === "group") return { kind: "group", name: identity.group };
	return { kind: "user", name: identity.username, groups: identity.groups };
}

/** The verifier's request identity: a blank namespace reads as `default`, a user without groups gets `identityDefaults`. */
export function reviewIdentity(identity: InspectorIdentity): RbacAccessReviewIdentity {
	if (identity.kind === "serviceAccount") return { kind: "serviceAccount", name: identity.name, namespace: identity.namespace || "default" };
	if (identity.kind === "group") return { kind: "group", group: identity.name };
	return { kind: "user", username: identity.name, groups: identity.groups?.length ? identity.groups : identityDefaults("user", identity.name) };
}

/** `verbs · groups/resources`, then `· names: …` when the rule is limited to named objects. */
export function grantSummary(grant: ObservedGrant): string {
	const summary = `${grant.verbs.join(", ")} · ${grant.apiGroups.join(", ") || "core"}/${grant.resources.join(", ") || grant.nonResourceUrls.join(", ")}`;
	return grant.resourceNames.length ? `${summary} · names: ${grant.resourceNames.join(", ")}` : summary;
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
